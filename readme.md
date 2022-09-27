# RTL-SDR
This is a library for interfacing a Realtek RTL2832-based DVB dongle as an SDR receiver. It is a port of the [Osmocom library](https://osmocom.org/projects/rtl-sdr/wiki), with changes from the [RTL-SDR Blog fork](https://github.com/rtlsdrblog/rtl-sdr-blog) available via a build flag.

While this project is functional for some radios it is still in development and doesn't have feature parity with the Osmocom library (contributions welcome!). Some items on the todo list: 
- [ ] Support for more tuners (currently only includes `r820t`)
- [ ] Support async USB transfers (waiting on support in [rusb](https://github.com/a1ien/rusb))

## Getting Started
A great way to see how to use this library is by looking at the [examples](/examples/). [simple_fm](examples/simple_fm.rs) is intended as an instructive minimal end-to-end example, showing the setup and demodulation process as clearly as possible. It is single-threaded for simplicity, so audio output may be choppy (will hopefully add a multi-threaded example in the future).