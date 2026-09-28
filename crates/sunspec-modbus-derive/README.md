**`sunspec-modbus-derive` provides the procedural macro implementation for  [sunspec-modbus-lib-rs](https://crates.io/crates/sunspec-modbus-lib-rs).** 

> ⚠️ **Note:** This is a supporting crate for [sunspec-modbus-lib-rs](https://crates.io/crates/sunspec-modbus-lib-rs), and
> should generally not be installed directly.

## `#[derive(ModelList)]` for `sunspec_modbus_lib_rs::ModelList`.

Derives a [`ModelList`](https://docs.rs/sunspec-modbus-lib-rs) impl for a struct of SunSpec models - either tuple or
named. This includes:
 * a ReadAdapters struct covering every model,
 * a WriteAdapters struct covering only the models that are writable,
 * traversable Iterator impls for each of the above that pair each model with its adapter, traversed in the order of
   definition in the source struct

## Usage
```rust
use sunspec_modbus_lib_rs::{
    ModelList, Sunspec,
    sunspec::models::{model_1, model_103, model_708},
};

const CURVE_COUNT: u16 = 2;
const POINT_COUNT: u16 = 3;

#[derive(ModelList)]
struct SunspecModels {
    model_1: model_1::Model1,
    model_103: model_103::Model103,
    model_708: model_708::Model708,
}

/// The device's register map: the common model, an inverter model, and a DER high-voltage-trip
/// curve model. The same list backs both reads and writes.
const SUNSPEC: Sunspec<SunspecModels> = Sunspec::new(SunspecModels {
    model_1: model_1::Model1,
    model_103: model_103::Model103,
    model_708: model_708::Model708 {
        stored_curve_count: CURVE_COUNT,
        number_of_points: POINT_COUNT,
    },
});
```

For more detailed usage see <https://github.com/cuprous-au/sunspec-modbus-codec/tree/main/crates/sunspec-modbus-lib-rs/examples>.