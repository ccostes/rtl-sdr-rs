use std::time::Duration;

use crate::error::Result;
use crate::error::RtlsdrError::RtlsdrErr;
use rusb::{Context, UsbContext};
use log::{error, info};

use super::KNOWN_DEVICES;
#[derive(Debug)]
pub struct DeviceHandle {
    handle: rusb::DeviceHandle<Context>,
}
impl DeviceHandle {
    pub fn open(index: usize) -> Result<Self> {
        let mut context = Context::new()?;
        let handle = DeviceHandle::open_device(&mut context, index)?;
        Ok(DeviceHandle { handle: handle })
    }
    pub fn open_device<T: UsbContext>(
        context: &mut T,
        index: usize,
    ) -> Result<rusb::DeviceHandle<T>> {
        let devices = context.devices().map_err(|e| {
            info!("Failed to get devices: {:?}", e);  // Logging with info!
            RtlsdrErr(format!("Error: {:?}", e))
        })?;
    
        let mut device_count = 0;
    
        // Iterate through the devices and check their descriptors
        for (i, found) in devices.iter().enumerate() {
            let device_desc = match found.device_descriptor() {
                Ok(desc) => desc,
                Err(e) => {
                    info!("Failed to get device descriptor for device {}: {:?}", i, e);  // Logging with info!
                    continue;
                }
            };

            for dev in KNOWN_DEVICES.iter() {
                if device_desc.vendor_id() == dev.vid && device_desc.product_id() == dev.pid {
                    info!(
                        "Found device at index {} Vendor ID = {:04x}, Product ID = {:04x}",
                        i, device_desc.vendor_id(), device_desc.product_id()
                    );
    
                    if device_count == index {
                        info!("Opening device at index {}", index);  // Logging with info!
                        return found.open().map_err(|e| {
                            info!("Failed to open device: {:?}", e);  // Logging with info!
                            RtlsdrErr(format!("Error: {:?}", e))
                        });
                    }
                    device_count += 1;
                }
            }
        }
    
        info!(
            "No matching device found at the requested index {}. Total matched devices: {}",
            index, device_count
        );  // Logging with info!
    
        Err(RtlsdrErr(format!(
            "No device found at index {}",
            index
        )))
    }
    
    pub fn claim_interface(&mut self, iface: u8) -> Result<()> {
        Ok(self.handle.claim_interface(iface)?)
    }
    pub fn reset(&mut self) -> Result<()> {
        Ok(self.handle.reset()?)
    }

    pub fn read_control(
        &self,
        request_type: u8,
        request: u8,
        value: u16,
        index: u16,
        buf: &mut [u8],
        timeout: Duration,
    ) -> Result<usize> {
        Ok(self
            .handle
            .read_control(request_type, request, value, index, buf, timeout)?)
    }

    pub fn write_control(
        &self,
        request_type: u8,
        request: u8,
        value: u16,
        index: u16,
        buf: &[u8],
        timeout: Duration,
    ) -> Result<usize> {
        Ok(self
            .handle
            .write_control(request_type, request, value, index, buf, timeout)?)
    }

    pub fn read_bulk(&self, endpoint: u8, buf: &mut [u8], timeout: Duration) -> Result<usize> {
        Ok(self.handle.read_bulk(endpoint, buf, timeout)?)
    }
}
