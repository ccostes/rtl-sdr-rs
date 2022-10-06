# Background
Before getting into the library details it useful to know some domain context. RTL-SDR encompasses a family of devices (see the `KNOWN_DEVICES` list in [device/constants.rs](device/constants.rs)) which share a common base but use a few different tuners which require unique configuration. 

# Library Structure
The layout of this library reflects the context above - an `RtlSdr` struct defined in  [rtlsdr.rs](rtlsdr.rs) contains the core logic, and includes a `tuner` field which is dynamically populated with one of the implementations in the [tuners](tuners/) module depending on which tuner is detected.

Generic USB and IO functionality is implemented in the [device/](device/) module.