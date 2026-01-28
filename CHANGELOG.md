# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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
