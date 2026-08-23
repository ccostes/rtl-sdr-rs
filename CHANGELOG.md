# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.4.0](https://github.com/ccostes/rtl-sdr-rs/compare/v0.3.3...v0.4.0) - 2026-08-23

### Other

- [**breaking**] migrate USB backend to nusb

### Added

- Continuous, cancellable IQ streaming through `RtlSdr::read_async`

### Changed

- **Breaking:** replace the public `rusb` error payload with backend-neutral
  `UsbError` and `UsbErrorKind` types; the next release must be 0.4.0
- Use `nusb` instead of `rusb` for USB enumeration, control transfers, and bulk
  transfers

## [0.3.3](https://github.com/ccostes/rtl-sdr-rs/compare/v0.3.2...v0.3.3) - 2026-05-31

### Other

- format codebase with rustfmt

## [0.3.2](https://github.com/ccostes/rtl-sdr-rs/compare/v0.3.1...v0.3.2) - 2026-05-31

### Other

- Fix `rtl_tcp` IPv6 listen address parsing
- Fix R82xx `read_gain` to return cumulative gain in tenths of dB
- Require tuner backends to be `Send + Sync`
- Add README badges

## [0.3.1](https://github.com/ccostes/rtl-sdr-rs/compare/v0.3.0...v0.3.1) - 2026-02-10

### Other

- Fix panic in _xtal_check
- Run cargo clippy --fix
- Propagate up USB errors instead of swallowing
- Reduce cost of opening / enumerating devices

## [0.3.0](https://github.com/ccostes/rtl-sdr-rs/compare/v0.2.1...v0.3.0) - 2026-01-28

### Added

- device enumeration and sensor API

### Other

- Update readme
- Fix issues from PR that maybe wasn't ready to merge - whoops
- eliminate scan-then-reopen
- select by index or by filters

## [0.2.1](https://github.com/ccostes/rtl-sdr-rs/compare/v0.2.0...v0.2.1) - 2025-11-02

### Fixed

- `div_buf_cur` assigned twice when `rtl_sdr_blog` feature is enabled

### Other

- Test all features in github automation
