use rtlsdr_rs::RtlSdrDevice;
use rusb::{Context, Device, DeviceHandle, Result, UsbContext, Error};
mod usb;
use usb::RtlSdrDeviceHandle;


fn main() -> Result<()> {
    let mut device = RtlSdrDevice::open();

    Ok(())
}
