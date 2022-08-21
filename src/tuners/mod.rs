pub mod r820t;

use super::RtlSdrDeviceHandle;

pub struct TunerInfo {
    pub id: &'static str,
    pub name: &'static str,
    pub i2c_addr: u8,
    pub check_addr: u8,
    pub check_val: u8,
    // pub gains: Vec<i8>,
}

pub trait Tuner {
    fn init(&self, handle: &mut RtlSdrDeviceHandle);
}