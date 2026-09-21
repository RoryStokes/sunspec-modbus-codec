#![no_std]

// `#[derive(ModelList)]`'s expansion refers to this crate by name (it has to: it's an external
// proc-macro crate with no other way to name the crate it's invoked from), which only resolves
// when the derive is used from a *different* crate. This self-alias makes it resolve here too,
// for the derive's own use in this crate's tests.
extern crate self as sunspec_modbus_lib_rs;

pub mod buffer;
pub mod cursor;
#[macro_use]
pub mod macros;
pub mod model;
pub mod sunspec;

pub use crate::model::{ModelList, ModelSpec, STARTING_REGISTER_OFFSET, StaticModelSpec, Sunspec};
pub use sunspec_modbus_derive::ModelList;

#[derive(Debug, Copy, Clone)]
pub enum ModbusException {
    IllegalFunction = 0x01,
    IllegalDataAddress = 0x02,
    IllegalDataValue = 0x03,
    ServerDeviceFailure = 0x04,
    Acknowledge = 0x05,
    ServerDeviceBusy = 0x06,
    MemoryParityError = 0x08,
    GatewayPathUnavailable = 0x0A,
    GatewayTargetDevice = 0x0B,
}

#[cfg(test)]
mod tests {
    use core::ffi::CStr;

    #[cfg(feature = "test-models")]
    use crate::sunspec::models::{model_701, model_704};
    use crate::sunspec::{
        adapters::{ReadBinding, WriteBinding},
        models::model_1::{self, Model1StatefulAdapter},
    };

    use super::*;

    #[test]
    fn simple_common_adapter() -> Result<(), ModbusException> {
        struct SunspecModel {
            model: model_1::Model1,
        }
        let mut adapter = Model1StatefulAdapter {
            manufacturer: c_char_array!("Cuprous"),
            model: c_char_array!("Inverter 1"),
            options: c_char_array!("opt_a_b_c"),
            version: c_char_array!("v0.1"),
            serial_number: c_char_array!("I-1"),
            device_address: 0,
        };

        impl ModelList for SunspecModel {
            type ReadAdapters<'a> = &'a Model1StatefulAdapter;

            type WriteAdapters<'a> = &'a mut Model1StatefulAdapter;

            fn read_iter<'a>(
                &'a self,
                adapter: Self::ReadAdapters<'a>,
            ) -> impl Iterator<Item = ReadBinding<'a>> {
                Some(ReadBinding::Model1(&self.model, adapter)).into_iter()
            }

            fn write_iter<'a>(
                &'a self,
                adapter: Self::WriteAdapters<'a>,
            ) -> impl Iterator<Item = WriteBinding<'a>> {
                Some(WriteBinding::Model1(&self.model, adapter)).into_iter()
            }
        }

        let sunspec = Sunspec::new(SunspecModel {
            model: model_1::Model1,
        });

        const WORDS_TO_READ: u16 = 72;

        let mut init_buf = [0_u8; WORDS_TO_READ as usize * 2];

        sunspec.read_registers(STARTING_REGISTER_OFFSET, init_buf.as_mut_slice(), &adapter)?;

        assert_eq!(&init_buf[..4], b"SunS");
        assert_eq!(
            u16::from_be_bytes(init_buf[4..6].try_into().expect("Unexpected slice length")),
            1
        );
        assert_eq!(
            u16::from_be_bytes(init_buf[6..8].try_into().expect("Unexpected slice length")),
            66
        );
        assert_eq!(CStr::from_bytes_until_nul(&init_buf[8..40]), Ok(c"Cuprous"));
        assert_eq!(
            CStr::from_bytes_until_nul(&init_buf[40..72]),
            Ok(c"Inverter 1")
        );
        assert_eq!(
            CStr::from_bytes_until_nul(&init_buf[72..88]),
            Ok(c"opt_a_b_c")
        );
        assert_eq!(CStr::from_bytes_until_nul(&init_buf[88..104]), Ok(c"v0.1"));
        assert_eq!(CStr::from_bytes_until_nul(&init_buf[104..136]), Ok(c"I-1"));
        assert_eq!(
            u16::from_be_bytes(
                init_buf[136..138]
                    .try_into()
                    .expect("Unexpected slice length")
            ),
            0
        );

        sunspec.write_multiple_registers(
            STARTING_REGISTER_OFFSET + 68,
            [1234u16].as_slice(),
            &mut adapter,
        )?;

        assert_eq!(model_1::ReadAdapter::device_address(&adapter), Some(1234));

        let mut after_buf = [0_u8; 40];

        sunspec.read_registers(
            STARTING_REGISTER_OFFSET + 52,
            after_buf.as_mut_slice(),
            &adapter,
        )?;

        assert_eq!(CStr::from_bytes_until_nul(&after_buf[0..32]), Ok(c"I-1"));
        assert_eq!(
            u16::from_be_bytes(
                after_buf[32..34]
                    .try_into()
                    .expect("Unexpected slice length")
            ),
            1234
        );
        Ok(())
    }

    #[test]
    #[cfg(feature = "test-models")]
    fn write_model_704_sets_active_power_enable() -> Result<(), ModbusException> {
        #[derive(ModelList)]
        struct SunspecModel {
            model_1: model_1::Model1,
            model_701: model_701::Model701,
            model_704: model_704::Model704,
        }

        let mut common_model = Model1StatefulAdapter {
            manufacturer: c_char_array!("Cuprous"),
            model: c_char_array!("Inverter 1"),
            options: c_char_array!("opt_a_b_c"),
            version: c_char_array!("v0.1"),
            serial_number: c_char_array!("I-1"),
            device_address: 0,
        };

        struct DerAcControlsModel {
            active_power_enable: bool,
        }

        impl model_704::WriteAdapter for DerAcControlsModel {
            fn set_active_power_enable(&mut self, value: model_704::WSetEna) {
                self.active_power_enable = value == model_704::WSetEna::Enabled;
            }
        }

        let sunspec = Sunspec::new(SunspecModel {
            model_1: model_1::Model1,
            model_701: model_701::Model701,
            model_704: model_704::Model704,
        });

        let mut der_ac_controls = DerAcControlsModel {
            active_power_enable: false,
        };

        let mut adapters = SunspecModelWriteAdapters {
            model_1: &mut common_model,
            model_704: &mut der_ac_controls,
        };

        sunspec.write_multiple_registers(
            40247,
            hex::decode("0001").unwrap().as_slice(),
            &mut adapters,
        )?;

        assert!(der_ac_controls.active_power_enable);

        Ok(())
    }

    /// `#[derive(ModelList)]` on a tuple struct: `model_701` (no writable points) sits between
    /// the two writable models, so a correct `WriteAdapters` must renumber its fields around the
    /// writable subset (positions 0, 1) rather than reusing the original struct's positions
    /// (0, 2) - otherwise this would either fail to compile or write through the wrong adapter.
    #[test]
    fn derived_tuple_struct_model_list_write_indexes_the_writable_subset() {
        #[derive(ModelList)]
        struct Trio(model_1::Model1, model_701::Model701, model_704::Model704);

        let model_list = Trio(model_1::Model1, model_701::Model701, model_704::Model704);

        let mut common_model = Model1StatefulAdapter {
            manufacturer: c_char_array!("Cuprous"),
            model: c_char_array!("Inverter 1"),
            options: c_char_array!("opt_a_b_c"),
            version: c_char_array!("v0.1"),
            serial_number: c_char_array!("I-1"),
            device_address: 0,
        };

        struct DerAcControlsModel {
            active_power_enable: bool,
        }

        impl model_704::WriteAdapter for DerAcControlsModel {
            fn set_active_power_enable(&mut self, value: model_704::WSetEna) {
                self.active_power_enable = value == model_704::WSetEna::Enabled;
            }
        }

        let mut der_ac_controls = DerAcControlsModel {
            active_power_enable: false,
        };

        let mut adapters = TrioWriteAdapters(&mut common_model, &mut der_ac_controls);
        let mut iter = model_list.write_iter(&mut adapters);
        match iter.next() {
            Some(WriteBinding::Model1(_, _)) => (),
            _ => panic!("Expected first binding to be model 1"),
        };
        match iter.next() {
            Some(WriteBinding::Model701(_)) => (),
            _ => panic!("Expected second binding to be model 701"),
        };
        match iter.next() {
            Some(WriteBinding::Model704(_, _)) => (),
            _ => panic!("Expected third binding to be model 704"),
        };
        assert!(iter.next().is_none());
    }

    /// Companion to the indexing test above: exercises the actual register write through a
    /// derived `ModelList`'s `Sunspec` wrapper, rather than just the binding order `write_iter`
    /// produces.
    #[test]
    fn derived_tuple_struct_model_list_write_reaches_the_right_adapter()
    -> Result<(), ModbusException> {
        #[derive(ModelList)]
        struct Trio(model_1::Model1, model_701::Model701, model_704::Model704);

        let model_list = Trio(model_1::Model1, model_701::Model701, model_704::Model704);

        let mut common_model = Model1StatefulAdapter {
            manufacturer: c_char_array!("Cuprous"),
            model: c_char_array!("Inverter 1"),
            options: c_char_array!("opt_a_b_c"),
            version: c_char_array!("v0.1"),
            serial_number: c_char_array!("I-1"),
            device_address: 0,
        };

        struct DerAcControlsModel {
            active_power_enable: bool,
        }

        impl model_704::WriteAdapter for DerAcControlsModel {
            fn set_active_power_enable(&mut self, value: model_704::WSetEna) {
                self.active_power_enable = value == model_704::WSetEna::Enabled;
            }
        }

        let mut der_ac_controls = DerAcControlsModel {
            active_power_enable: false,
        };

        let sunspec = Sunspec::new(model_list);
        sunspec.write_multiple_registers(
            40247,
            hex::decode("0001").unwrap().as_slice(),
            &mut TrioWriteAdapters(&mut common_model, &mut der_ac_controls),
        )?;

        assert!(der_ac_controls.active_power_enable);

        Ok(())
    }

    /// A repeating model's C-facing `StatefulPtrAdapter`, driven through the `StaticModelSpec`
    /// vtable, must read and write exactly like the const-generic `StatefulAdapter` it mirrors.
    #[cfg(feature = "test-models")]
    mod stateful_ptr_adapter {
        use core::ffi::c_void;
        use core::mem::zeroed;

        use crate::buffer::{ReadableRegisterBuffer, WritableRegisterBuffer};
        use crate::cursor::Cursor;
        use crate::model::ModelSpec;
        use crate::sunspec::models::model_708::{
            Model708, Model708MayTripCurveMayTripCurvePoints,
            Model708MayTripCurveMayTripCurvePointsPtr,
            Model708MomentaryCessationCurveMomCessationCurvePoints,
            Model708MomentaryCessationCurveMomCessationCurvePointsPtr,
            Model708MustTripCurveMustTripCurvePoints, Model708MustTripCurveMustTripCurvePointsPtr,
            Model708StatefulAdapter, Model708StatefulPtrAdapter, Model708StoredCurves,
            Model708StoredCurvesPtr, SUNSPEC_MODEL_708,
        };

        const CURVES: usize = 2;
        const POINTS: usize = 3;
        const MODEL: Model708 = Model708 {
            stored_curve_count: CURVES as u16,
            number_of_points: POINTS as u16,
        };
        const STATEFUL: u8 = 1;

        /// Distinct, index-derived values so a mis-indexed group shows up as a mismatch.
        fn value(salt: usize, crv: usize, pt: usize) -> u16 {
            (salt * 1000 + crv * 100 + pt * 10 + 1) as u16
        }

        // SAFETY (whole fn): all-zero is a valid `Model708StatefulAdapter` - integers, and
        // `repr(u16)` enums whose zero variant exists.
        fn inline_adapter() -> Model708StatefulAdapter<CURVES, POINTS> {
            let mut adapter: Model708StatefulAdapter<CURVES, POINTS> = unsafe { zeroed() };
            adapter.adopt_curve_request = 7;
            adapter.number_of_points = POINTS as u16;
            adapter.stored_curve_count = CURVES as u16;
            adapter.voltage_scale_factor = -1;
            adapter.time_point_scale_factor = -2;
            for crv in 0..CURVES {
                let curve: &mut Model708StoredCurves<POINTS> = &mut adapter.stored_curves[crv];
                curve.must_trip_curve_crv_number_of_active_points = value(1, crv, 0);
                curve.may_trip_curve_crv_number_of_active_points = value(2, crv, 0);
                curve.momentary_cessation_curve_crv_number_of_active_points = value(3, crv, 0);
                for pt in 0..POINTS {
                    let must: &mut Model708MustTripCurveMustTripCurvePoints =
                        &mut curve.must_trip_curve_must_trip_curve_points[pt];
                    must.must_trip_curve_pt_voltage_point = value(4, crv, pt);
                    must.must_trip_curve_pt_time_point = value(5, crv, pt) as u32;
                    let may: &mut Model708MayTripCurveMayTripCurvePoints =
                        &mut curve.may_trip_curve_may_trip_curve_points[pt];
                    may.may_trip_curve_pt_voltage_point = value(6, crv, pt);
                    may.may_trip_curve_pt_time_point = value(7, crv, pt) as u32;
                    let momentary: &mut Model708MomentaryCessationCurveMomCessationCurvePoints =
                        &mut curve.momentary_cessation_curve_mom_cessation_curve_points[pt];
                    momentary.momentary_cessation_curve_pt_voltage_point = value(8, crv, pt);
                    momentary.momentary_cessation_curve_pt_time_point = value(9, crv, pt) as u32;
                }
            }
            adapter
        }

        /// Owns the arrays a `Model708StatefulPtrAdapter` points into.
        struct PtrStorage {
            curves: [Model708StoredCurvesPtr; CURVES],
            must: [[Model708MustTripCurveMustTripCurvePointsPtr; POINTS]; CURVES],
            may: [[Model708MayTripCurveMayTripCurvePointsPtr; POINTS]; CURVES],
            momentary:
                [[Model708MomentaryCessationCurveMomCessationCurvePointsPtr; POINTS]; CURVES],
        }

        /// Builds storage and an adapter wired to it, holding the same values as
        /// [`inline_adapter`] (or zeroed, when `filled` is false).
        fn ptr_adapter(storage: &mut PtrStorage, filled: bool) -> Model708StatefulPtrAdapter {
            // SAFETY: as for `inline_adapter`; pointers zero to null and are set below.
            *storage = unsafe { zeroed() };
            let salted = |salt, crv, pt| if filled { value(salt, crv, pt) } else { 0 };
            for crv in 0..CURVES {
                storage.curves[crv].must_trip_curve_crv_number_of_active_points = salted(1, crv, 0);
                storage.curves[crv].may_trip_curve_crv_number_of_active_points = salted(2, crv, 0);
                storage.curves[crv].momentary_cessation_curve_crv_number_of_active_points =
                    salted(3, crv, 0);
                for pt in 0..POINTS {
                    storage.must[crv][pt].must_trip_curve_pt_voltage_point = salted(4, crv, pt);
                    storage.must[crv][pt].must_trip_curve_pt_time_point = salted(5, crv, pt) as u32;
                    storage.may[crv][pt].may_trip_curve_pt_voltage_point = salted(6, crv, pt);
                    storage.may[crv][pt].may_trip_curve_pt_time_point = salted(7, crv, pt) as u32;
                    storage.momentary[crv][pt].momentary_cessation_curve_pt_voltage_point =
                        salted(8, crv, pt);
                    storage.momentary[crv][pt].momentary_cessation_curve_pt_time_point =
                        salted(9, crv, pt) as u32;
                }
                storage.curves[crv].must_trip_curve_must_trip_curve_points =
                    storage.must[crv].as_mut_ptr();
                storage.curves[crv].may_trip_curve_may_trip_curve_points =
                    storage.may[crv].as_mut_ptr();
                storage.curves[crv].momentary_cessation_curve_mom_cessation_curve_points =
                    storage.momentary[crv].as_mut_ptr();
            }
            // SAFETY: as above.
            let mut adapter: Model708StatefulPtrAdapter = unsafe { zeroed() };
            if filled {
                adapter.adopt_curve_request = 7;
                adapter.number_of_points = POINTS as u16;
                adapter.stored_curve_count = CURVES as u16;
                adapter.voltage_scale_factor = -1;
                adapter.time_point_scale_factor = -2;
            }
            adapter.stored_curves = storage.curves.as_mut_ptr();
            adapter
        }

        #[test]
        fn read_matches_inline_stateful_adapter() {
            let length = MODEL.model_length();
            let mut expected = [0u16; 256];
            let mut actual = [0u16; 256];
            let inline = inline_adapter();
            MODEL
                .traverse_points_read(
                    &inline,
                    &mut WritableRegisterBuffer::from(&mut expected[..length as usize]),
                    0,
                )
                .unwrap();

            // SAFETY: zeroed storage is valid (see `inline_adapter`).
            let mut storage: PtrStorage = unsafe { zeroed() };
            let adapter = ptr_adapter(&mut storage, true);
            let mut cursor = Cursor::new(0, length);
            // SAFETY: `adapter` is a live `Model708StatefulPtrAdapter` whose arrays hold
            // `CURVES` curves of `POINTS` points, matching the repeat counts passed.
            unsafe {
                (SUNSPEC_MODEL_708.visit_read)(
                    STATEFUL,
                    &adapter as *const _ as *const c_void,
                    CURVES as u16,
                    POINTS as u16,
                    &mut cursor,
                    &mut WritableRegisterBuffer::from(&mut actual[..length as usize]),
                );
            }

            assert!(cursor.error.is_none());
            assert_eq!(expected[..length as usize], actual[..length as usize]);
            // Not vacuously equal: the model's own values came through.
            assert!(actual[..length as usize].contains(&value(6, 1, 2)));
        }

        /// Replays every one- and two-word write at every offset of the model against both
        /// adapters, so the read-only, one-word and two-word (`u32`) points all get exercised
        /// without hard-coding the layout: each write must be accepted or rejected identically,
        /// and leave identical state behind.
        #[test]
        fn write_matches_inline_stateful_adapter() {
            let length = MODEL.model_length() as usize;
            let mut image = [0u16; 256];
            MODEL
                .traverse_points_read(
                    &inline_adapter(),
                    &mut WritableRegisterBuffer::from(&mut image[..length]),
                    0,
                )
                .unwrap();

            // SAFETY: zeroed adapters are valid (see `inline_adapter`).
            let mut inline: Model708StatefulAdapter<CURVES, POINTS> = unsafe { zeroed() };
            let mut storage: PtrStorage = unsafe { zeroed() };
            let mut adapter = ptr_adapter(&mut storage, false);

            let mut accepted = 0;
            for offset in 0..length {
                for words in 1..=2.min(length - offset) {
                    let request = &image[offset..offset + words];
                    let inline_result = MODEL.traverse_points_write(
                        &mut inline,
                        &ReadableRegisterBuffer::from(request),
                        offset as u16,
                    );

                    let mut cursor = Cursor::new(offset as u16, words as u16);
                    // SAFETY: as for the read test, with `adapter` uniquely borrowed.
                    unsafe {
                        (SUNSPEC_MODEL_708.visit_write)(
                            STATEFUL,
                            &mut adapter as *mut _ as *mut c_void,
                            CURVES as u16,
                            POINTS as u16,
                            &mut cursor,
                            &ReadableRegisterBuffer::from(request),
                        );
                    }
                    assert_eq!(
                        inline_result.is_ok(),
                        cursor.error.is_none(),
                        "write of {words} word(s) at offset {offset}"
                    );
                    accepted += usize::from(inline_result.is_ok());
                }
            }

            assert!(accepted > 0);
            assert_eq!(inline.adopt_curve_request, adapter.adopt_curve_request);
            for crv in 0..CURVES {
                let expected = &inline.stored_curves[crv];
                let actual = &storage.curves[crv];
                assert_eq!(
                    expected.must_trip_curve_crv_number_of_active_points,
                    actual.must_trip_curve_crv_number_of_active_points
                );
                assert_eq!(
                    expected.momentary_cessation_curve_crv_number_of_active_points,
                    actual.momentary_cessation_curve_crv_number_of_active_points
                );
                for pt in 0..POINTS {
                    assert_eq!(
                        expected.must_trip_curve_must_trip_curve_points[pt]
                            .must_trip_curve_pt_voltage_point,
                        storage.must[crv][pt].must_trip_curve_pt_voltage_point
                    );
                    assert_eq!(
                        expected.may_trip_curve_may_trip_curve_points[pt]
                            .may_trip_curve_pt_time_point,
                        storage.may[crv][pt].may_trip_curve_pt_time_point
                    );
                    assert_eq!(
                        expected.momentary_cessation_curve_mom_cessation_curve_points[pt]
                            .momentary_cessation_curve_pt_voltage_point,
                        storage.momentary[crv][pt].momentary_cessation_curve_pt_voltage_point
                    );
                }
            }
            // Not vacuous: writes really landed, in the deepest group of the last curve.
            assert_eq!(
                storage.may[CURVES - 1][POINTS - 1].may_trip_curve_pt_voltage_point,
                value(6, CURVES - 1, POINTS - 1)
            );
            assert_eq!(
                storage.momentary[CURVES - 1][POINTS - 1].momentary_cessation_curve_pt_time_point,
                u32::from(value(9, CURVES - 1, POINTS - 1))
            );
        }
    }
}
