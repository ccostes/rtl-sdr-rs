use rtlsdr_rs::RtlSdr;
use rusb::{Context, Device, DeviceHandle, Result, UsbContext, Error};
mod usb;
use usb::RtlSdrDeviceHandle;

fn main() -> Result<()> {
    // Open device
    let mut sdr = RtlSdr::open();

    // Set the tuner gain
    sdr.set_tuner_gain_mode(rtlsdr_rs::TunerGainMode::AUTO);

    Ok(())
}
