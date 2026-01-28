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

### Device Enumeration

List and identify devices before opening them:

```rust
// List all connected RTL-SDR devices
let devices = RtlSdr::list_devices()?;
for device in devices {
    println!("Device {}: Serial {}", device.index, device.serial);
}

// Open by serial number (great for multi-device setups!)
let sdr = RtlSdr::open_with_serial("00000001")?;

// Or just open the first available device
let sdr = RtlSdr::open_first_available()?;
```

See the [device_list example](examples/device_list.rs) for a complete working demonstration.

### Linux: Kernel Modules

**⚠️ Linux users:** If you get a `Usb(Busy)` error, the DVB-T kernel modules need to be unloaded:

```bash
sudo rmmod rtl2832_sdr dvb_usb_rtl28xxu rtl2832 rtl8xxxu
```

**Why?** RTL-SDR dongles are detected as DVB-T TV receivers by Linux. The kernel modules claim exclusive USB access, preventing userspace applications (like this library) from accessing the device.

**Solutions:**

**Temporary:** Run the `rmmod` commands above before each use. Note that modules will reload on next device plug or system reboot.

**Permanent (Linux):** Blacklist the modules to prevent them from loading automatically. Create `/etc/modprobe.d/blacklist-rtlsdr.conf`:

```bash
# Blacklist RTL-SDR DVB-T kernel drivers to allow userspace access
blacklist dvb_usb_rtl28xxu
blacklist rtl2832
blacklist rtl2832_sdr
blacklist rtl8xxxu
```

Then update initramfs and reboot:
```bash
# Debian/Ubuntu:
sudo update-initramfs -u

# RHEL/Fedora/CentOS:
sudo dracut --force

# Arch:
sudo mkinitcpio -P

# Then reboot
sudo reboot
```

**Alternative:** Use [SoapySDR](https://github.com/pothosware/SoapySDR) which works with the kernel's DVB-T driver interface (`/dev/dvb/*` device nodes) instead of bypassing it. This allows it to work with kernel modules loaded, but adds abstraction layers that can impact performance. SoapySDR is best for end-user applications where you can't unload modules, while this library provides better performance and full hardware control when modules are unloaded.

The example is thoroughly documented to clearly show how to use this library, and hopefully make the FM demodulation process understandable too!

## Build Options
This library includes the RTL-SDR Blog [modifications](https://github.com/rtlsdrblog/rtl-sdr-blog) to the original Osmocom library as a feature. Enable it in cargo with the `--features rtl_sdr_blog` flag.

## Supported Tuners

- **Rafael Micro R820T**
  - Common tuner found in RTL-SDR V3 and many DVB-T dongles
  - Frequency range: 24 - 1766 MHz
  
- **Rafael Micro R828D**  
  - Found in RTL-SDR Blog V4
  - Frequency range: 24 - 1766 MHz
  - Automatically detects RTL-SDR Blog V4 hardware via USB strings
  - Blog V4 features:
    - 28.8 MHz upconverter support with automatic frequency translation
    - Automatic input switching for HF/VHF/UHF bands
    - Notch filter control based on frequency

## Contributing
Contributions to this project are welcome! Check out the [Issues page](https://github.com/ccostes/rtl-sdr-rs/issues) to see what's on the roadmap that you could help with, or open a new Issue.

## Acknowledgments
This library originated as a port of the [Osmocom rtl-sdr library](https://osmocom.org/projects/rtl-sdr/wiki), with modifications from the [RTL-SDR Blog fork](https://github.com/rtlsdrblog/rtl-sdr-blog).
