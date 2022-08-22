use rtlsdr_rs::RtlSdr;
use rusb::{Context, Device, DeviceHandle, Result, UsbContext, Error};
mod usb;
use usb::RtlSdrDeviceHandle;

fn main() -> Result<()> {
    // Open device
    let mut sdr = RtlSdr::open();

    // Set the tuner gain
    sdr.set_tuner_gain_mode(rtlsdr_rs::TunerGainMode::AUTO);
    // Reset the endpoint before we try to read from it (mandatory)
    sdr.reset_buffer();
    // set up primary channel
    sdr.set_center_freq(120_900_000);

    Ok(())
}
