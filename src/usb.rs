use rusb::{Context, DeviceHandle, Result, Error};
use byteorder::{ByteOrder, LittleEndian, BigEndian};
use std::time::Duration;

#[cfg(test)]
#[path = "usb_test.rs"]
mod usb_test;

const DEF_RTL_XTAL_FREQ: u32 = 28800000; // should this be here?

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
#[derive(Debug)]
pub struct RtlSdrDeviceHandle {
    handle: DeviceHandle<Context>,
}

impl RtlSdrDeviceHandle {
    pub fn new(handle: DeviceHandle<Context>) -> RtlSdrDeviceHandle {
            RtlSdrDeviceHandle { handle: handle }
    }

    pub fn print_device_info(&self) -> Result<()> {
        let device_desc = self.handle.device().device_descriptor()?;
        let timeout = Duration::from_secs(1);
        let languages = self.handle.read_languages(timeout)?;
    
        println!("Active configurations: {}", self.handle.active_configuration()?);
    
        if !languages.is_empty() {
            let language = languages[0];
            // println!("Language: {:?}", language);
    
            println!(
                "Manufacturer: {}",
                self.handle.read_manufacturer_string(language, &device_desc, timeout)
                .unwrap_or("Not Found".to_string())
            );
            println!(
                "Product: {}",
                self.handle
                    .read_product_string(language, &device_desc, timeout)
                    .unwrap_or("Not Found".to_string())
            );
            println!(
                "Serial Number: {}",
                self.handle
                    .read_serial_number_string(language, &device_desc, timeout)
                    .unwrap_or("Not Found".to_string())
            );
        }
        Ok(())
    }

    pub fn set_if_freq(&self, freq: u32) {
        // Get corrected clock value - start with default
        let rtl_xtal: u32 = DEF_RTL_XTAL_FREQ;
        // Apply PPM correction
        let base = 1u32 << 22;
        let if_freq: i32 = (freq as f64 * base as f64 / rtl_xtal as f64 * -1f64) as i32;

        let tmp = ((if_freq >> 16) as u16) & 0x3f;
        self.demod_write_reg(1, 0x19, tmp, 1);
        let tmp = ((if_freq >> 8) as u16) & 0xff;
        self.demod_write_reg(1, 0x1a, tmp, 1);
        let tmp = if_freq as u16 & 0xff;
        self.demod_write_reg(1, 0x1b, tmp, 1);
    }

    pub fn claim_interface(&mut self, iface: u8) {
        self.handle.claim_interface(iface);
    }

    pub fn reset_demod(&self){
        self.demod_write_reg(1, 0x01, 0x14, 1);
        self.demod_write_reg(1, 0x01, 0x10, 1);
    }

    pub fn test_write(&mut self) {
        // try a dummy write and reset device if it fails
        let len: usize = self.write_reg(BLOCK_USB, USB_SYSCTL, 0x09, 1);
        if len == 0 {
            println!("Resetting device...");
            self.handle.reset();
        }
    }

    pub fn read_array(&self, block: u16, addr: u16, arr: &mut [u8], _len: u8) -> usize {
        let index: u16 = block << 8;
        self.handle.read_control(CTRL_IN, 0, addr, index, arr, CTRL_TIMEOUT).unwrap()
    }
    
    pub fn write_array(&self, block: u16, addr: u16, arr: &[u8], len: usize) -> Result<usize> {
        let index: u16 = (block << 8) | 0x10;
        self.handle.write_control(CTRL_OUT, 0, addr, index, &arr[..len], CTRL_TIMEOUT)
    }
    
    pub fn i2c_read_reg(&self, i2c_addr: u8, reg: u8) -> Result<u8> {
        let addr: u16 = i2c_addr.into();
        let reg: [u8; 1] = [reg];
        let mut data: [u8; 1] = [0];
    
        match self.write_array(BLOCK_IIC, addr, &reg, 1) {
            Ok(_res) => {
                self.read_array(BLOCK_IIC, addr, &mut data, 1);
                Ok(data[0])
            }
            Err(e) => Err(e),
        }
    }
    
    pub fn i2c_write(&self, i2c_addr: u16, buffer: &[u8]) {
        self.write_array(BLOCK_IIC, i2c_addr, buffer, buffer.len());
    }
    
    pub fn i2c_read(&self, i2c_addr: u16, buffer: &mut[u8], len: u8) {
        self.read_array(BLOCK_IIC, i2c_addr, buffer, len);
    }

    pub fn read_eeprom(&self, mut data: &[u8], offset: u8, len: usize) {
        assert!(len + offset as usize <= 256); // TODO: maybe not an assert here?
        self.write_array(BLOCK_IIC, EEPROM_ADDR, &[offset], 1);
        for i in 0..len {
            self.read_array(BLOCK_IIC, EEPROM_ADDR, &mut [data[i]], 1);
        }
    }

    pub fn read_reg(&self, block: u16, addr: u16, len: usize) -> u16 {
        let mut data: [u8;2] = [0,0];
        let index: u16 = block << 8;
        self.handle.read_control(CTRL_IN, 0, addr, index, &mut data[..len], CTRL_TIMEOUT);
        BigEndian::read_u16(&data)
    }

    pub fn write_reg(&self, block: u16, addr: u16, val: u16, len: usize) -> usize {
        let data: [u8; 2] = val.to_be_bytes();
        let data_slice = if len == 1 {
            &data[1..2]
        } else {
            &data
        };
        let index = (block << 8) | 0x10;
        // println!("write_reg addr: {:x} index: {:x} data: {:x?} data slice: {}", addr, index, data, data_slice.len());
        match self.handle.write_control(
            CTRL_OUT, 0, addr, index, data_slice, CTRL_TIMEOUT) {
            Ok(n) => n,
            Err(e) => {
                println!("write_reg failed: {} block: {block} addr: {addr} val: {val}", e);
                0
            }
        }
    }

    pub fn demod_read_reg(&self, page: u16, addr: u16) -> u16 {
        let mut data: [u8; 2] = [0, 0];
        let index = page;
        let _bytes = match self.handle.read_control(
                CTRL_IN, 0, (addr << 8) | 0x20, index, &mut data, CTRL_TIMEOUT) {
                    Ok(n) => {
                        // println!("demod_read_reg got {} bytes: [{:#02x}, {:#02x}] value: {:x}", n, data[0], data[1], BigEndian::read_u16(&data));
                        n
                    },
                    Err(e) => {
                        println!("demod_read_reg failed: {} page: {:#02x} addr: {:#02x}", e, page, addr);
                        0
                    }
                };
        let reg: u16 = BigEndian::read_u16(&data);
        reg
    }

    pub fn demod_write_reg(&self, page: u16, mut addr: u16, val: u16, len: usize) -> usize {
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
                println!("demod_write_reg failed: {} page: {:#02x} addr: {:#02x} val: {:#02x}", e, page, addr, val);
                0
            }
        };
        
        self.demod_read_reg(0x0a, 0x1);
            
        bytes
    }

    pub fn bulk_transfer(&self, buf: &mut [u8]) -> Result<usize> {
        self.handle.read_bulk(0x81, buf, Duration::ZERO)
    }
}