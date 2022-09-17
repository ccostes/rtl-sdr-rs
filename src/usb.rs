/// Low-level io functions for interfacing with rusb(libusb)

use log::{info, error};
use rusb::{Context, DeviceHandle};
use byteorder::{ByteOrder, BigEndian};
use std::time::Duration;
use crate::error::Result;

// use mockall::*;
// use mockall::predicate::*;
#[cfg(test)]
#[path = "usb_test.rs"]
mod usb_test;

const EEPROM_ADDR: u16 = 0xa0;
pub const EEPROM_SIZE: usize = 256;

// Blocks
pub const BLOCK_DEMOD: u16  = 0;
pub const BLOCK_USB: u16    = 1;
pub const BLOCK_SYS: u16    = 2;
pub const BLOCK_TUN: u16    = 3;
pub const BLOCK_ROM: u16    = 4;
pub const BLOCK_IRB: u16    = 5;
pub const BLOCK_IIC: u16    = 6;

// Sys Registers
pub const DEMOD_CTL: u16    = 0x3000;
pub const GPO: u16          = 0x3001;
pub const GPI: u16          = 0x3002;
pub const GPOE: u16         = 0x3003;
pub const GPD: u16          = 0x3004;
pub const SYSINTE: u16      = 0x3005;
pub const SYSINTS: u16      = 0x3006;
pub const GP_CFG0: u16      = 0x3007;
pub const GP_CFG1: u16      = 0x3008;
pub const SYSINTE_1: u16    = 0x3009;
pub const SYSINTS_1: u16    = 0x300a;
pub const DEMOD_CTL_1: u16  = 0x300b;
pub const IR_SUSPEND: u16   = 0x300c;

// USB Registers
pub const USB_SYSCTL: u16       = 0x2000;
pub const USB_CTRL: u16         = 0x2010;
pub const USB_STAT: u16         = 0x2014;
pub const USB_EPA_CFG: u16      = 0x2144;
pub const USB_EPA_CTL: u16      = 0x2148;
pub const USB_EPA_MAXPKT: u16   = 0x2158;
pub const USB_EPA_MAXPKT_2: u16 = 0x215a;
pub const USB_EPA_FIFO_CFG: u16 = 0x2160;

const CTRL_IN: u8 = rusb::constants::LIBUSB_ENDPOINT_IN | rusb::constants::LIBUSB_REQUEST_TYPE_VENDOR;
const CTRL_OUT: u8 = rusb::constants::LIBUSB_ENDPOINT_OUT | rusb::constants::LIBUSB_REQUEST_TYPE_VENDOR;
const CTRL_TIMEOUT: Duration = Duration::from_millis(300);

// pub trait UsbDevice {
//     fn new(handle: DeviceHandle<Context>) -> DeviceHandle;

// }
#[derive(Debug)]
pub struct RtlSdrDeviceHandle {
    handle: DeviceHandle<Context>,
}

impl RtlSdrDeviceHandle {
    pub fn new(handle: DeviceHandle<Context>) -> RtlSdrDeviceHandle {
            RtlSdrDeviceHandle { handle: handle }
    }

    pub fn claim_interface(&mut self, iface: u8) -> Result<()> {
        self.handle.claim_interface(iface)?;
        Ok(())
    }

    pub fn reset_demod(&self) -> Result<()> {
        self.demod_write_reg(1, 0x01, 0x14, 1)?;
        self.demod_write_reg(1, 0x01, 0x10, 1)?;
        Ok(())
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

    pub fn read_array(&self, block: u16, addr: u16, arr: &mut [u8], _len: u8) -> Result<usize> {
        let index: u16 = block << 8;
        Ok(self.handle.read_control(CTRL_IN, 0, addr, index, arr, CTRL_TIMEOUT)?)
    }
    
    pub fn write_array(&self, block: u16, addr: u16, arr: &[u8], len: usize) -> Result<usize> {
        let index: u16 = (block << 8) | 0x10;
        Ok(self.handle.write_control(CTRL_OUT, 0, addr, index, &arr[..len], CTRL_TIMEOUT)?)
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
    
    pub fn i2c_read(&self, i2c_addr: u16, buffer: &mut[u8], len: u8) -> Result<usize> {
        self.read_array(BLOCK_IIC, i2c_addr, buffer, len)
    }

    pub fn read_eeprom(&self, data: &[u8], offset: u8, len: usize) -> Result<usize> {
        assert!(len + offset as usize <= 256); // TODO: maybe not an assert here?
        self.write_array(BLOCK_IIC, EEPROM_ADDR, &[offset], 1)?;
        for i in 0..len {
            self.read_array(BLOCK_IIC, EEPROM_ADDR, &mut [data[i]], 1)?;
        }
        Ok(len)
    }

    pub fn read_reg(&self, block: u16, addr: u16, len: usize) -> Result<u16> {
        let mut data: [u8;2] = [0,0];
        let index: u16 = block << 8;
        self.handle.read_control(CTRL_IN, 0, addr, index, &mut data[..len], CTRL_TIMEOUT)?;
        Ok(BigEndian::read_u16(&data))
    }

    pub fn write_reg(&self, block: u16, addr: u16, val: u16, len: usize) -> Result<usize> {
        let data: [u8; 2] = val.to_be_bytes();
        let data_slice = if len == 1 {
            &data[1..2]
        } else {
            &data
        };
        let index = (block << 8) | 0x10;
        // info!("write_reg addr: {:x} index: {:x} data: {:x?} data slice: {}", addr, index, data, data_slice.len());
        Ok(self.handle.write_control(CTRL_OUT, 0, addr, index, data_slice, CTRL_TIMEOUT)?)
    }

    pub fn demod_read_reg(&self, page: u16, addr: u16) -> Result<u16> {
        let mut data: [u8; 2] = [0, 0];
        let index = page;
        let _bytes = match self.handle.read_control(
                CTRL_IN, 0, (addr << 8) | 0x20, index, &mut data, CTRL_TIMEOUT) {
                    Ok(n) => {
                        // info!("demod_read_reg got {} bytes: [{:#02x}, {:#02x}] value: {:x}", n, data[0], data[1], BigEndian::read_u16(&data));
                        Ok(n)
                    },
                    Err(e) => {
                        error!("demod_read_reg failed: {} page: {:#02x} addr: {:#02x}", e, page, addr);
                        Err(e)
                    }
                };
        let reg: u16 = BigEndian::read_u16(&data);
        Ok(reg)
    }

    pub fn demod_write_reg(&self, page: u16, mut addr: u16, val: u16, len: usize) -> Result<usize> {
        let index = 0x10 | page;
        addr = (addr << 8) | 0x20; 
        let data: [u8; 2] = val.to_be_bytes();
        let data_slice = if len == 1 {
            &data[1..2]
        } else {
            &data
        };

        let bytes = match self.handle.write_control(
            CTRL_OUT, 0, addr, index, data_slice, CTRL_TIMEOUT) {
            Ok(n) => n,
            Err(e) => {
                error!("demod_write_reg failed: {} page: {:#02x} addr: {:#02x} val: {:#02x}", e, page, addr, val);
                0
            }
        };
        
        self.demod_read_reg(0x0a, 0x1)?;
            
        Ok(bytes)
    }

    pub fn bulk_transfer(&self, buf: &mut [u8]) -> Result<usize> {
        Ok(self.handle.read_bulk(0x81, buf, Duration::ZERO)?)
    }
}