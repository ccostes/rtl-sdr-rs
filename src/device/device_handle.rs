// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use std::time::Duration;

use crate::error::Result;
use crate::error::RtlsdrError::RtlsdrErr;
use crate::DeviceId;
use log::info;
use rusb::{Context, UsbContext};

enum UsbSelector<'a> {
    Index(usize),
    Serial(&'a str),
}

#[derive(Debug)]
pub struct DeviceHandle {
    handle: rusb::DeviceHandle<Context>,
}
impl DeviceHandle {
    pub fn open(device_id: DeviceId) -> Result<Self> {
        let mut context = Context::new()?;
        match device_id {
            DeviceId::Fd(fd) => DeviceHandle::open_device_with_fd(&mut context, fd),
            DeviceId::Index(idx) => {
                DeviceHandle::open_from_usb(&mut context, UsbSelector::Index(idx))
            }
            DeviceId::Serial(s) => {
                DeviceHandle::open_from_usb(&mut context, UsbSelector::Serial(s))
            }
        }
        .map(|handle| DeviceHandle { handle })
    }

    fn open_from_usb(
        context: &mut Context,
        selector: UsbSelector,
    ) -> Result<rusb::DeviceHandle<Context>> {
        let devices = context.devices().map_err(|e| {
            info!("Failed to get devices: {:?}", e);
            RtlsdrErr(format!("Error: {:?}", e))
        })?;

        let mut current_idx = 0;

        for device in devices.iter() {
            let desc = match device.device_descriptor() {
                Ok(d) => d,
                Err(_) => continue,
            };

            if !crate::device::is_known_device(desc.vendor_id(), desc.product_id()) {
                continue;
            }

            match selector {
                UsbSelector::Index(target_idx) => {
                    if current_idx == target_idx {
                        info!("Opening device at index {}", target_idx);
                        return device.open().map_err(|e| {
                            info!("Failed to open device: {:?}", e);
                            RtlsdrErr(format!("Error: {:?}", e))
                        });
                    }
                    current_idx += 1;
                }
                UsbSelector::Serial(target_serial) => match device.open() {
                    Ok(handle) => {
                        let sn_index = desc.serial_number_string_index();
                        if let Some(idx) = sn_index {
                            if let Ok(s) = handle.read_string_descriptor_ascii(idx) {
                                if s == *target_serial {
                                    info!("Opening device with serial {}", target_serial);
                                    return Ok(handle);
                                }
                            }
                        }
                    }
                    Err(e) => {
                        info!("Failed to check serial on candidate device: {:?}", e);
                    }
                },
            }
        }

        let msg = match selector {
            UsbSelector::Index(i) => format!("No device found at index {}", i),
            UsbSelector::Serial(s) => format!("No device found with serial {}", s),
        };

        Err(RtlsdrErr(msg))
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
        Err(RtlsdrErr(
            "File descriptor opening is only supported on Unix systems".to_string(),
        ))
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

    pub fn get_usb_strings(&self) -> Result<(Option<String>, Option<String>, Option<String>)> {
        let device = self.handle.device();
        let descriptor = device
            .device_descriptor()
            .map_err(|e| RtlsdrErr(format!("Failed to read device descriptor: {e}")))?;

        let read_string = |index: Option<u8>| -> Result<Option<String>> {
            match index {
                Some(i) => self
                    .handle
                    .read_string_descriptor_ascii(i)
                    .map(Some)
                    .map_err(|e| RtlsdrErr(format!("Failed to read string descriptor: {e}"))),
                None => Ok(None),
            }
        };

        let manufacturer = read_string(descriptor.manufacturer_string_index())?;
        let product = read_string(descriptor.product_string_index())?;
        let serial = read_string(descriptor.serial_number_string_index())?;

        Ok((manufacturer, product, serial))
    }
}
