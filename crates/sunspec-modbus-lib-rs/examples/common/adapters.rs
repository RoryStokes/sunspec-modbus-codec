use core::{
    ffi::CStr,
    sync::atomic::{AtomicU16, Ordering},
};

use sunspec_modbus_lib_rs::sunspec::models::{model_1, model_103, model_708};

/// The device's entire mutable state. Everything else served below (manufacturer strings,
/// amperage, frequency, ...) is fixed, so there's nothing else to hold.
pub static DEVICE_ADDRESS: AtomicU16 = AtomicU16::new(0);
pub static VOLTAGE_AN: AtomicU16 = AtomicU16::new(0);
pub static VOLTAGE_BN: AtomicU16 = AtomicU16::new(0);
pub static VOLTAGE_CN: AtomicU16 = AtomicU16::new(0);

/// Backs `model_1`'s adapters. Zero-sized: its one piece of state, the device address,
/// lives in [`DEVICE_ADDRESS`] rather than a struct field.
pub struct CommonModel;

impl model_1::ReadAdapter for CommonModel {
    fn manufacturer(&self) -> &CStr {
        c"Cuprous"
    }

    fn model(&self) -> &CStr {
        c"Inverter 1"
    }

    fn options(&self) -> Option<&CStr> {
        Some(c"opt_a_b_c")
    }

    fn version(&self) -> Option<&CStr> {
        Some(c"v0.1")
    }

    fn serial_number(&self) -> &CStr {
        c"I-1"
    }

    fn device_address(&self) -> Option<u16> {
        Some(DEVICE_ADDRESS.load(Ordering::Relaxed))
    }
}

impl model_1::WriteAdapter for CommonModel {
    fn set_device_address(&mut self, value: u16) {
        DEVICE_ADDRESS.store(value, Ordering::Relaxed);
    }
}

/// Backs `model_103`'s read adapter. Zero-sized: its mutable state, the per-phase
/// voltages, lives in [`VOLTAGE_AN`] / [`VOLTAGE_BN`] / [`VOLTAGE_CN`] rather than a
/// struct field, refreshed on a timer by [`randomise_voltages`].
pub struct InverterModel;

impl model_103::ReadAdapter for InverterModel {
    fn amps(&self) -> u16 {
        0
    }

    fn amps_phase_a(&self) -> u16 {
        1
    }

    fn amps_phase_b(&self) -> u16 {
        1
    }

    fn amps_phase_c(&self) -> u16 {
        1
    }

    fn a_sf(&self) -> i16 {
        -1
    }

    fn phase_voltage_an(&self) -> u16 {
        VOLTAGE_AN.load(Ordering::Relaxed)
    }

    fn phase_voltage_bn(&self) -> u16 {
        VOLTAGE_BN.load(Ordering::Relaxed)
    }

    fn phase_voltage_cn(&self) -> u16 {
        VOLTAGE_CN.load(Ordering::Relaxed)
    }

    fn v_sf(&self) -> i16 {
        -1
    }

    fn watts(&self) -> i16 {
        1
    }

    fn w_sf(&self) -> i16 {
        1
    }

    fn hz(&self) -> u16 {
        1234
    }

    fn hz_sf(&self) -> i16 {
        -2
    }

    fn watt_hours(&self) -> u32 {
        1
    }

    fn wh_sf(&self) -> i16 {
        0
    }

    fn cabinet_temperature(&self) -> i16 {
        1
    }

    fn tmp_sf(&self) -> i16 {
        0
    }

    fn operating_state(&self) -> model_103::St {
        model_103::St::Standby
    }

    fn event1(&self) -> u32 {
        1
    }

    fn event_bitfield_2(&self) -> u32 {
        1
    }
}

/// Two stored curve sets (`NCrvSet`) of three points (`NPt`) each, for `model_708`'s
/// must-trip/may-trip/momentary-cessation curves. Register values are whole percentages
/// (`voltage_scale_factor` 0) and tenths of a second (`time_point_scale_factor` -1).
pub const CURVE_COUNT: u16 = 2;
pub const POINT_COUNT: u16 = 3;

/// A (voltage, time) point on a synthetic curve, decreasing in voltage and increasing in trip
/// time as `pt_index`/`crv_index` grow - enough shape to be a plausible curve without claiming
/// to be a real compliance one.
fn curve_point(
    base_voltage_pct: u16,
    base_time_tenths: u32,
    crv_index: u16,
    pt_index: u16,
) -> (u16, u32) {
    let voltage = base_voltage_pct - crv_index * 5 - pt_index * 5;
    let time = base_time_tenths + u32::from(crv_index) * 5 + u32::from(pt_index) * 20;
    (voltage, time)
}

/// Backs `model_708`'s adapters: [`CURVE_COUNT`] stored curve sets of [`POINT_COUNT`] points
/// each, computed by [`curve_point`]. Its two writable points (`Ena`, `AdptCrvReq`) round-trip
/// through [`MODULE_ENABLED`] / [`ADOPT_CURVE_REQUEST`]; every curve/point setter is optional
/// and left at its no-op default, since this example doesn't support reconfiguring curves.
pub struct CurveModel;

static MODULE_ENABLED: AtomicU16 = AtomicU16::new(model_708::Ena::Enabled as u16);
static ADOPT_CURVE_REQUEST: AtomicU16 = AtomicU16::new(0);

impl model_708::ReadAdapter for CurveModel {
    fn der_trip_hv_module_enable(&self) -> model_708::Ena {
        if MODULE_ENABLED.load(Ordering::Relaxed) == model_708::Ena::Enabled as u16 {
            model_708::Ena::Enabled
        } else {
            model_708::Ena::Disabled
        }
    }

    fn adopt_curve_request(&self) -> u16 {
        ADOPT_CURVE_REQUEST.load(Ordering::Relaxed)
    }

    fn adopt_curve_result(&self) -> model_708::AdptCrvRslt {
        model_708::AdptCrvRslt::Completed
    }

    fn number_of_points(&self) -> u16 {
        POINT_COUNT
    }

    fn stored_curve_count(&self) -> u16 {
        CURVE_COUNT
    }

    fn voltage_scale_factor(&self) -> i16 {
        0
    }

    fn time_point_scale_factor(&self) -> i16 {
        -1
    }

    fn crv_curve_access(&self, _crv_index: u16) -> model_708::ReadOnly {
        model_708::ReadOnly::Rw
    }

    fn must_trip_curve_crv_number_of_active_points(&self, _crv_index: u16) -> Option<u16> {
        Some(POINT_COUNT)
    }

    fn may_trip_curve_crv_number_of_active_points(&self, _crv_index: u16) -> Option<u16> {
        Some(POINT_COUNT)
    }

    fn momentary_cessation_curve_crv_number_of_active_points(
        &self,
        _crv_index: u16,
    ) -> Option<u16> {
        Some(POINT_COUNT)
    }

    fn must_trip_curve_pt_voltage_point(&self, crv_index: u16, pt_index: u16) -> Option<u16> {
        Some(curve_point(120, 2, crv_index, pt_index).0)
    }

    fn must_trip_curve_pt_time_point(&self, crv_index: u16, pt_index: u16) -> Option<u32> {
        Some(curve_point(120, 2, crv_index, pt_index).1)
    }

    fn may_trip_curve_pt_voltage_point(&self, crv_index: u16, pt_index: u16) -> Option<u16> {
        Some(curve_point(115, 5, crv_index, pt_index).0)
    }

    fn may_trip_curve_pt_time_point(&self, crv_index: u16, pt_index: u16) -> Option<u32> {
        Some(curve_point(115, 5, crv_index, pt_index).1)
    }

    fn momentary_cessation_curve_pt_voltage_point(
        &self,
        crv_index: u16,
        pt_index: u16,
    ) -> Option<u16> {
        Some(curve_point(125, 1, crv_index, pt_index).0)
    }

    fn momentary_cessation_curve_pt_time_point(
        &self,
        crv_index: u16,
        pt_index: u16,
    ) -> Option<u32> {
        Some(curve_point(125, 1, crv_index, pt_index).1)
    }
}

impl model_708::WriteAdapter for CurveModel {
    fn set_der_trip_hv_module_enable(&mut self, value: model_708::Ena) {
        MODULE_ENABLED.store(value as u16, Ordering::Relaxed);
    }

    fn set_adopt_curve_request(&mut self, value: u16) {
        ADOPT_CURVE_REQUEST.store(value, Ordering::Relaxed);
    }
}
