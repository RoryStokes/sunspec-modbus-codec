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
