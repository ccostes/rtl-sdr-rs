pub mod r820t;
use crate::device::Device;
use crate::error::Result;
use crate::TunerGain;

pub const KNOWN_TUNERS: [TunerInfo; 1] = [r820t::TUNER_INFO];

#[derive(Debug, Clone, Copy)]

pub struct TunerInfo {
    pub id: &'static str,
    pub name: &'static str,
    pub i2c_addr: u8,
    pub check_addr: u8,
    pub check_val: u8,
    // pub gains: Vec<i8>,
}

pub trait Tuner: std::fmt::Debug {
    fn init(&mut self, handle: &Device) -> Result<()>;
    fn get_info(&self) -> Result<TunerInfo>;
    fn get_gains(&self) -> Result<Vec<i32>>;
    fn read_gain(&self, handle: &Device) -> Result<i32>;
    fn set_gain(&mut self, handle: &Device, gain: TunerGain) -> Result<()>;
    fn set_freq(&mut self, handle: &Device, freq: u32) -> Result<()>;
    fn set_bandwidth(&mut self, handle: &Device, bw: u32, rate: u32) -> Result<()>;
    fn get_if_freq(&self) -> Result<u32>;
    fn get_xtal_freq(&self) -> Result<u32>;
    fn set_xtal_freq(&mut self, freq: u32) -> Result<()>;
    fn exit(&mut self, handle: &Device) -> Result<()>;
}
#[derive(Debug)]
pub struct NoTuner {}
impl Tuner for NoTuner {
    fn init(&mut self, _handle: &Device) -> Result<()> {
        Ok(())
    }
    fn get_info(&self) -> Result<TunerInfo> {
        Ok(TunerInfo {
            id: "",
            name: "",
            i2c_addr: 0,
            check_addr: 0,
            check_val: 0,
        })
    }
    fn get_gains(&self) -> Result<Vec<i32>> {
        Ok(vec![])
    }
    fn read_gain(&self, _handle: &Device) -> Result<i32> {
        Ok(0)
    }
    fn set_gain(&mut self, _handle: &Device, _gain: TunerGain) -> Result<()> {
        Ok(())
    }
    fn set_freq(&mut self, _handle: &Device, _freq: u32) -> Result<()> {
        Ok(())
    }
    fn set_bandwidth(&mut self, _handle: &Device, _bw: u32, _rate: u32) -> Result<()> {
        Ok(())
    }
    fn get_xtal_freq(&self) -> Result<u32> {
        Ok(0)
    }
    fn set_xtal_freq(&mut self, _freq: u32) -> Result<()> {
        Ok(())
    }
    fn get_if_freq(&self) -> Result<u32> {
        Ok(0)
    }
    fn exit(&mut self, _handle: &Device) -> Result<()> {
        Ok(())
    }
}
