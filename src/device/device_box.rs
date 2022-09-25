use std::fmt;
use super::*;

pub struct DeviceBox(pub Box<dyn Device>);
impl Device for DeviceBox {
    fn claim_interface(&mut self, iface: u8) -> Result<()> {
        self.0.claim_interface(iface)
    }
    fn test_write(&mut self) -> Result<()> {
        self.0.test_write()
    }
    fn reset_demod(&self) -> Result<()> {
        self.0.reset_demod()
    }
    fn read_reg(&self, block: u16, addr: u16, len: usize) -> Result<u16>{
        self.0.read_reg(block, addr, len)
    }
    fn write_reg(&self, block: u16, addr: u16, val: u16, len: usize) -> Result<usize>{
        self.0.write_reg(block, addr, val, len)
    }
    fn demod_read_reg(&self, page: u16, addr: u16) -> Result<u16>{
        self.0.demod_read_reg(page, addr)
    }
    fn demod_write_reg(&self, page: u16, mut addr: u16, val: u16, len: usize) -> Result<usize>{
        self.0.demod_write_reg(page, addr, val, len)
    }
    fn bulk_transfer(&self, buf: &mut [u8]) -> Result<usize> {
        self.0.bulk_transfer(buf)
    }
    fn read_eeprom(&self, data: &[u8], offset: u8, len: usize) -> Result<usize>{
        self.0.read_eeprom(data, offset, len)
    }
    fn i2c_read_reg(&self, i2c_addr: u8, reg: u8) -> Result<u8>{
        self.0.i2c_read_reg(i2c_addr, reg)
    }
    fn i2c_write(&self, i2c_addr: u16, buffer: &[u8]) -> Result<usize>{
        self.0.i2c_write(i2c_addr, buffer)
    }
    fn i2c_read(&self, i2c_addr: u16, buffer: &mut[u8], len: u8) -> Result<usize>{
        self.0.i2c_read(i2c_addr, buffer, len)
    }
}
impl fmt::Debug for DeviceBox {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "DeviceBox")
    }
}