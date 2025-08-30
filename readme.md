# RTL-SDR
An RTL-SDR library written in Rust!

## What is RTL-SDR?
RTL-SDR is a family of low-cost (~$30) USB software-defined radio (SDR) receivers that can tune a wide range of frequencies which are then processed in software (thus the 'software' in SDR).

They can receive all kinds of signals such as FM radio (see the [simple_fm example](examples/) in this project), aircraft radio and position data (like what you see on [adsb-exchange](https://globe.adsbexchange.com/)), weather satellite imagery, and more!

[rtl-sdr.com](https://www.rtl-sdr.com/about-rtl-sdr/) has a great page with much more explanation.
## Getting Started
You can run the example [FM radio receiver](examples/simple_fm.rs) with the following command on Mac:
```
cargo run --example simple_fm | play -r 32k -t raw -e s -b 16 -c 1 -V1 -
```
and similarly on Linux:
```
cargo run --example simple_fm | aplay -r 32000 -f S16_LE
```

### Opening Devices
This library supports multiple ways to open RTL-SDR devices:

#### By device index (default method):
```rust
use rtl_sdr_rs::{DeviceId, RtlSdr};

// Method 1: Using DeviceId enum
let sdr = RtlSdr::open(DeviceId::Index(0))?;

// Method 2: Using convenience function 
let sdr = RtlSdr::open_with_index(0)?;
```

#### By file descriptor (useful on Android):
```rust
use rtl_sdr_rs::{DeviceId, RtlSdr};

// Method 1: Using DeviceId enum
let sdr = RtlSdr::open(DeviceId::Fd(fd))?;

// Method 2: Using convenience function
let sdr = RtlSdr::open_with_fd(fd)?;
```

See the [demo_device_id example](examples/demo_device_id.rs) for a complete demonstration of all opening methods.
### Uload Kernel Modules
If the RTL kernel modules are installed you will need to temporarily unload them before using this library as follows:
```
sudo rmmod rtl2832_sdr
sudo rmmod dvb_usb_rtl28xxu
sudo rmmod rtl2832
sudo rmmod rtl8xxxu
```
Failure to do so will result in the following USB error:
```
thread 'main' panicked at 'Unable to open SDR device!: Usb(Busy)'
```

The example is thoroughly documented to clearly show how to use this library, and hopefully make the FM demodulation process understandable too!

## Build Options
This library includes the RTL-SDR Blog [modifications](https://github.com/rtlsdrblog/rtl-sdr-blog) to the original Osmocom library as a feature. Enable it in cargo with the `--features rtl_sdr_blog` flag.

## Contributing
Contributions to this project are welcome! Check out the [Issues page](https://github.com/ccostes/rtl-sdr-rs/issues) to see what's on the roadmap that you could help with, or open a new Issue.

## Acknowledgments
This library originated as a port of the [Osmocom rtl-sdr library](https://osmocom.org/projects/rtl-sdr/wiki), with modifications from the [RTL-SDR Blog fork](https://github.com/rtlsdrblog/rtl-sdr-blog).
