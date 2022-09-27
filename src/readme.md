This library is a functional port of the original [Osmocom library](https://osmocom.org/projects/rtl-sdr/wiki), with some significant structure and organization refactors, as well as comments,documentation and tests. The goal is understandable and idiomatic Rust code.

# Library Structure
## lib.rs
The public interface, and the bulk of the code overall, is in [lib.rs](lib.rs). While there is some re-organization overall vs. the original, `lib.rs` in particular could use some additional organization and separation into smaller files - it's a bit of a kitchen sink.

## Tuners
There are a number of different tuners that could be encountered, so the idea is to abstract them with an interface defined in [tuners/mod.rs](tuners/mod.rs). Currently the `R820T` tuner is the only one that has been implemented.

## Device
USB IO functionality is abstracted by the Device interface, defined in [device/mod.rs](device/mod.rs).