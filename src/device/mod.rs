pub mod constants;
pub use constants::*;
pub mod device_handle;
#[cfg(test)]
mod mock_device_handle;

#[cfg(not(test))]
use device_handle::DeviceHandle;
#[cfg(test)]
use mock_device_handle::MockDeviceHandle as DeviceHandle;

use crate::error::Result;
use byteorder::{ByteOrder, LittleEndian};
/// Low-level io functions for interfacing with rusb(libusb)
use log::{error, info};
use std::time::Duration;

#[cfg(test)]
mod device_test;

#[derive(Debug)]
pub struct Device {
    handle: DeviceHandle,
}

impl Device {
    pub fn new(index: usize) -> Result<Device> {
        Ok(Device {
            handle: DeviceHandle::open(index)?,
        })
    }

    pub fn claim_interface(&mut self, iface: u8) -> Result<()> {
        Ok(self.handle.claim_interface(iface)?)
    }

    pub fn test_write(&mut self) -> Result<()> {
        // try a dummy write and reset device if it fails
        let len: usize = self.write_reg(BLOCK_USB, USB_SYSCTL, 0x09, 1)?;
        if len == 0 {
            info!("Resetting device...");
            self.handle.reset()?;
        }
        Ok(())
    }

    pub fn reset_demod(&self) -> Result<()> {
        self.demod_write_reg(1, 0x01, 0x14, 1)?;
        self.demod_write_reg(1, 0x01, 0x10, 1)?;
        Ok(())
    }

    /// TODO: This only supports len of 1 or 2, maybe use an enum or make this generic?
    pub fn read_reg(&self, block: u16, addr: u16, len: usize) -> Result<u16> {
        assert!(len == 1 || len == 2);
        let mut data: [u8; 2] = [0, 0];
        let index: u16 = block << 8;
        self.handle
            .read_control(CTRL_IN, 0, addr, index, &mut data[..len], CTRL_TIMEOUT)?;
        // Read registers as little endian, but write as big; not sure why
        Ok(LittleEndian::read_u16(&data))
    }

    pub fn write_reg(&self, block: u16, addr: u16, val: u16, len: usize) -> Result<usize> {
        assert!(len == 1 || len == 2);
        // Read registers as little endian, but write as big; not sure why
        let data: [u8; 2] = val.to_be_bytes();
        let data_slice = if len == 1 { &data[1..2] } else { &data };
        let index = (block << 8) | 0x10;
        // info!("write_reg addr: {:x} index: {:x} data: {:x?} data slice: {}", addr, index, data, data_slice.len());
        Ok(self
            .handle
            .write_control(CTRL_OUT, 0, addr, index, data_slice, CTRL_TIMEOUT)?)
    }

    /// Only supports u8 reads
    pub fn demod_read_reg(&self, page: u16, addr: u16) -> Result<u16> {
        let mut data = [0_u8];
        let index = page;
        let _bytes = match self.handle.read_control(
            CTRL_IN,
            0,
            (addr << 8) | 0x20,
            index,
            &mut data,
            CTRL_TIMEOUT,
        ) {
            Ok(n) => {
                // info!("demod_read_reg got {} bytes: [{:#02x}, {:#02x}] value: {:x}", n, data[0], data[1], BigEndian::read_u16(&data));
                Ok(n)
            }
            Err(e) => {
                error!(
                    "demod_read_reg failed: {} page: {:#02x} addr: {:#02x}",
                    e, page, addr
                );
                Err(e)
            }
        };
        let reg: u16 = data[0] as u16;
        Ok(reg)
    }

    /// TODO: only supports len of 1 or 2, maybe use enum or make this generic
    pub fn demod_write_reg(&self, page: u16, mut addr: u16, val: u16, len: usize) -> Result<usize> {
        assert!(len == 1 || len == 2);
        let index = 0x10 | page;
        addr = (addr << 8) | 0x20;
        let data: [u8; 2] = val.to_be_bytes();
        let data_slice = if len == 1 { &data[1..2] } else { &data };

        let bytes =
            match self
                .handle
                .write_control(CTRL_OUT, 0, addr, index, data_slice, CTRL_TIMEOUT)
            {
                Ok(n) => n,
                Err(e) => {
                    error!(
                        "demod_write_reg failed: {} page: {:#02x} addr: {:#02x} val: {:#02x}",
                        e, page, addr, val
                    );
                    0
                }
            };

        self.demod_read_reg(0x0a, 0x1)?;

        Ok(bytes)
    }

    pub fn bulk_transfer(&self, buf: &mut [u8]) -> Result<usize> {
        Ok(self.handle.read_bulk(0x81, buf, Duration::ZERO)?)
    }

    pub fn read_eeprom(&self, data: &[u8], offset: u8, len: usize) -> Result<usize> {
        assert!((len + offset as usize) <= EEPROM_SIZE);
        self.write_array(BLOCK_IIC, EEPROM_ADDR, &[offset], 1)?;
        for i in 0..len {
            self.read_array(BLOCK_IIC, EEPROM_ADDR, &mut [data[i]], 1)?;
        }
        Ok(len)
    }

    pub fn i2c_read_reg(&self, i2c_addr: u8, reg: u8) -> Result<u8> {
        let addr: u16 = i2c_addr.into();
        let reg: [u8; 1] = [reg];
        let mut data: [u8; 1] = [0];

        match self.write_array(BLOCK_IIC, addr, &reg, 1) {
            Ok(_res) => {
                self.read_array(BLOCK_IIC, addr, &mut data, 1)?;
                Ok(data[0])
            }
            Err(e) => Err(e),
        }
    }

    pub fn i2c_write(&self, i2c_addr: u16, buffer: &[u8]) -> Result<usize> {
        Ok(self.write_array(BLOCK_IIC, i2c_addr, buffer, buffer.len())?)
    }

    pub fn i2c_read(&self, i2c_addr: u16, buffer: &mut [u8], len: u8) -> Result<usize> {
        self.read_array(BLOCK_IIC, i2c_addr, buffer, len)
    }

    pub fn read_array(&self, block: u16, addr: u16, arr: &mut [u8], _len: u8) -> Result<usize> {
        let index: u16 = block << 8;
        Ok(self
            .handle
            .read_control(CTRL_IN, 0, addr, index, arr, CTRL_TIMEOUT)?)
    }

    pub fn write_array(&self, block: u16, addr: u16, arr: &[u8], len: usize) -> Result<usize> {
        let index: u16 = (block << 8) | 0x10;
        Ok(self
            .handle
            .write_control(CTRL_OUT, 0, addr, index, &arr[..len], CTRL_TIMEOUT)?)
    }
}
