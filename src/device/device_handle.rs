// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

#![cfg_attr(test, allow(dead_code))]

use std::time::Duration;

use crate::error::Result;
use crate::error::RtlsdrError;
use crate::error::RtlsdrError::RtlsdrErr;
use crate::DeviceId;
use log::info;
use nusb::transfer::{Buffer, Bulk, ControlIn, ControlOut, ControlType, In, Recipient};
use nusb::{Device, Endpoint, Interface, MaybeFuture};

type UsbStrings = (Option<String>, Option<String>, Option<String>);

enum UsbSelector<'a> {
    Index(usize),
    Serial(&'a str),
}

#[derive(Debug)]
pub struct DeviceHandle {
    device: Device,
    interface: Option<Interface>,
    usb_strings: UsbStrings,
}
impl DeviceHandle {
    pub fn open(device_id: DeviceId) -> Result<Self> {
        let (device, usb_strings) = match device_id {
            DeviceId::Fd(fd) => DeviceHandle::open_device_with_fd(fd).map(|device| {
                let strings = read_device_strings(&device);
                (device, strings)
            }),
            DeviceId::Index(idx) => DeviceHandle::open_from_usb(UsbSelector::Index(idx)),
            DeviceId::Serial(s) => DeviceHandle::open_from_usb(UsbSelector::Serial(s)),
        }?;
        Ok(DeviceHandle {
            device,
            interface: None,
            usb_strings,
        })
    }

    fn open_from_usb(selector: UsbSelector) -> Result<(Device, UsbStrings)> {
        let devices = nusb::list_devices().wait().map_err(|e| {
            info!("Failed to get devices: {:?}", e);
            RtlsdrError::from_usb(e)
        })?;

        let mut current_idx = 0;

        for device in devices {
            if !crate::device::is_known_device(device.vendor_id(), device.product_id()) {
                continue;
            }

            match selector {
                UsbSelector::Index(target_idx) => {
                    if current_idx == target_idx {
                        info!("Opening device at index {}", target_idx);
                        let strings = device_strings(&device);
                        return device
                            .open()
                            .wait()
                            .map(|device| (device, strings))
                            .map_err(|e| {
                                info!("Failed to open device: {:?}", e);
                                RtlsdrError::from_usb(e)
                            });
                    }
                    current_idx += 1;
                }
                UsbSelector::Serial(target_serial) => {
                    if device.serial_number() == Some(target_serial) {
                        info!("Opening device with serial {}", target_serial);
                        let strings = device_strings(&device);
                        return device
                            .open()
                            .wait()
                            .map(|device| (device, strings))
                            .map_err(|e| {
                                info!("Failed to open device: {:?}", e);
                                RtlsdrError::from_usb(e)
                            });
                    }
                }
            }
        }

        let msg = match selector {
            UsbSelector::Index(i) => format!("No device found at index {}", i),
            UsbSelector::Serial(s) => format!("No device found with serial {}", s),
        };

        Err(RtlsdrErr(msg))
    }

    #[cfg(any(target_os = "linux", target_os = "android"))]
    pub fn open_device_with_fd(fd: i32) -> Result<Device> {
        use std::os::fd::BorrowedFd;

        info!("Opening device with file descriptor {}", fd);

        let fd = unsafe { BorrowedFd::borrow_raw(fd) }.try_clone_to_owned()?;
        let device = Device::from_fd(fd).wait().map_err(RtlsdrError::from_usb)?;
        Ok(device)
    }

    #[cfg(not(any(target_os = "linux", target_os = "android")))]
    pub fn open_device_with_fd(_fd: i32) -> Result<Device> {
        Err(RtlsdrErr(
            "File descriptor opening with nusb is only supported on Linux/Android".to_string(),
        ))
    }

    pub fn claim_interface(&mut self, iface: u8) -> Result<()> {
        self.interface = Some(
            self.device
                .claim_interface(iface)
                .wait()
                .map_err(RtlsdrError::from_usb)?,
        );
        Ok(())
    }
    pub fn reset(&mut self) -> Result<()> {
        self.device.reset().wait().map_err(RtlsdrError::from_usb)
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
        let direction = request_type & 0x80;
        if direction == 0 {
            return Err(RtlsdrErr(format!(
                "read_control called with OUT request type {request_type:#04x}"
            )));
        }
        let data = self
            .claimed_interface()?
            .control_in(
                ControlIn {
                    control_type: ControlType::Vendor,
                    recipient: Recipient::Device,
                    request,
                    value,
                    index,
                    length: buf.len().try_into().map_err(|_| {
                        RtlsdrErr(format!("control read length {} exceeds u16", buf.len()))
                    })?,
                },
                timeout,
            )
            .wait()
            .map_err(RtlsdrError::from_timed_usb_transfer)?;
        let n = data.len().min(buf.len());
        buf[..n].copy_from_slice(&data[..n]);
        Ok(n)
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
        let direction = request_type & 0x80;
        if direction != 0 {
            return Err(RtlsdrErr(format!(
                "write_control called with IN request type {request_type:#04x}"
            )));
        }
        self.claimed_interface()?
            .control_out(
                ControlOut {
                    control_type: ControlType::Vendor,
                    recipient: Recipient::Device,
                    request,
                    value,
                    index,
                    data: buf,
                },
                timeout,
            )
            .wait()
            .map_err(RtlsdrError::from_timed_usb_transfer)?;
        Ok(buf.len())
    }

    pub fn read_bulk(&self, endpoint: u8, buf: &mut [u8], timeout: Duration) -> Result<usize> {
        let interface = self.claimed_interface()?;
        let mut endpoint = interface
            .endpoint::<Bulk, In>(endpoint)
            .map_err(RtlsdrError::from_usb)?;
        endpoint.submit(Buffer::new(buf.len()));
        let completion = loop {
            match endpoint.wait_next_complete(read_poll_timeout(timeout)) {
                Some(completion) => break completion,
                None if timeout.is_zero() => continue,
                None => {
                    endpoint.cancel_all();
                    while endpoint.pending() > 0 {
                        let _ = endpoint.wait_next_complete(Duration::from_millis(100));
                    }
                    return Err(RtlsdrError::from_timed_usb_transfer(
                        nusb::transfer::TransferError::Cancelled,
                    ));
                }
            }
        };
        completion.status.map_err(RtlsdrError::from_usb_transfer)?;
        let n = completion.actual_len.min(buf.len());
        buf[..n].copy_from_slice(&completion.buffer[..n]);
        Ok(n)
    }

    pub fn get_usb_strings(&self) -> Result<(Option<String>, Option<String>, Option<String>)> {
        Ok(self.usb_strings.clone())
    }

    pub fn read_async(
        &self,
        endpoint: u8,
        buf_num: usize,
        buf_len: usize,
        cancel: &crate::async_read::CancelHandle,
        callback: &mut dyn FnMut(&[u8]),
    ) -> Result<()> {
        let interface = self.claimed_interface()?;
        let mut endpoint = interface
            .endpoint::<Bulk, In>(endpoint)
            .map_err(RtlsdrError::from_usb)?;
        crate::async_read::read_async_blocking(&mut endpoint, buf_num, buf_len, cancel, callback)
    }

    pub(crate) fn bulk_in_endpoint(&self, endpoint: u8) -> Result<Endpoint<Bulk, In>> {
        self.claimed_interface()?
            .endpoint::<Bulk, In>(endpoint)
            .map_err(RtlsdrError::from_usb)
    }

    fn claimed_interface(&self) -> Result<&Interface> {
        self.interface
            .as_ref()
            .ok_or_else(|| RtlsdrErr("USB interface has not been claimed".to_string()))
    }
}

fn device_strings(device: &nusb::DeviceInfo) -> UsbStrings {
    (
        device.manufacturer_string().map(str::to_string),
        device.product_string().map(str::to_string),
        device.serial_number().map(str::to_string),
    )
}

fn read_device_strings(device: &Device) -> UsbStrings {
    let descriptor = device.device_descriptor();
    let language_id = device
        .get_string_descriptor_supported_languages(Duration::from_secs(1))
        .wait()
        .ok()
        .and_then(|mut languages| languages.next())
        .unwrap_or(nusb::descriptors::language_id::US_ENGLISH);

    let read_string = |index: Option<std::num::NonZeroU8>| {
        index.and_then(|index| {
            device
                .get_string_descriptor(index, language_id, Duration::from_secs(1))
                .wait()
                .ok()
        })
    };

    (
        read_string(descriptor.manufacturer_string_index()),
        read_string(descriptor.product_string_index()),
        read_string(descriptor.serial_number_string_index()),
    )
}

fn read_poll_timeout(timeout: Duration) -> Duration {
    if timeout.is_zero() {
        Duration::from_millis(100)
    } else {
        timeout
    }
}
