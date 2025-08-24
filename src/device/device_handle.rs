// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use std::time::Duration;

use crate::Args;
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
    pub fn open(args: Args) -> Result<Self> {
        let mut context = Context::new()?;
        let handle = match args {
            Args::Index(index) => DeviceHandle::open_device(&mut context, index)?,
            Args::Fd(fd) => DeviceHandle::open_device_with_fd(&mut context, fd)?,
        };
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

    #[cfg(unix)]
    pub fn open_device_with_fd<T: UsbContext>(
        context: &mut T,
        fd: i32,
    ) -> Result<rusb::DeviceHandle<T>> {
        use std::os::unix::io::RawFd;
        
        info!("Opening device with file descriptor {}", fd);
        
        unsafe {
            context.open_device_with_fd(fd as RawFd).map_err(|e| {
                info!("Failed to open device with fd {}: {:?}", fd, e);
                RtlsdrErr(format!("Error opening device with fd {}: {:?}", fd, e))
            })
        }
    }

    #[cfg(not(unix))]
    pub fn open_device_with_fd<T: UsbContext>(
        _context: &mut T,
        _fd: i32,
    ) -> Result<rusb::DeviceHandle<T>> {
        Err(RtlsdrErr("File descriptor opening is only supported on Unix systems".to_string()))
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
