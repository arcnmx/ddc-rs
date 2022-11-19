# ddc

[![release-badge][]][cargo] [![docs-badge][]][docs] [![license-badge][]][license]

`ddc` is a Rust crate for controlling monitors with [DDC/CI](https://en.wikipedia.org/wiki/Display_Data_Channel).

## Implementations

`ddc` only provides traits for working with DDC, and these must be implemented
with an underlying backend in order to be used. The following crates may be
helpful:

- [ddc-i2c](https://crates.io/crates/ddc-i2c) supports DDC using an I2C capable
  master - in particular Linux's i2c-dev.
- [ddc-winapi](https://crates.io/crates/ddc-winapi) implements DDC using the
  Windows API. It is more limited than the generic I2C interface, and cannot be
  used to read monitor EDID info.
- [Any other downstream crates](https://crates.io/crates/ddc/reverse_dependencies)

## [Documentation][docs]

See the [documentation][docs] for up to date information. The [examples](examples/)
are a good place to start.

[release-badge]: https://img.shields.io/crates/v/ddc.svg?style=flat-square
[cargo]: https://crates.io/crates/ddc
[docs-badge]: https://img.shields.io/badge/API-docs-blue.svg?style=flat-square
[docs]: https://docs.rs/ddc/
[license-badge]: https://img.shields.io/badge/license-MIT-ff69b4.svg?style=flat-square
[license]: https://github.com/arcnmx/ddc-rs/blob/master/COPYING
