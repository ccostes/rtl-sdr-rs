pub mod r820t;

use super::*;

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

pub trait Tuner {
    fn init(&self, handle: &RtlSdrDeviceHandle);
    fn get_info(&self) -> TunerInfo;
    fn set_gain_mode(&mut self, handle: &RtlSdrDeviceHandle, mode: TunerGainMode);
    fn set_freq(&mut self, handle: &RtlSdrDeviceHandle, freq: u32);
    fn set_bandwidth(&mut self, handle: &RtlSdrDeviceHandle, bw: u32, rate: u32);
    fn get_if_freq(&self) -> u32;
}

pub struct NoTuner {}
impl Tuner for NoTuner {
    fn init(&self, handle: &RtlSdrDeviceHandle) {}
    fn get_info(&self) -> TunerInfo { TunerInfo { id: "", name: "", i2c_addr: 0, check_addr: 0, check_val: 0 } }
    fn set_gain_mode(&mut self, handle: &RtlSdrDeviceHandle, mode: TunerGainMode){}
    fn set_freq(&mut self, handle: &RtlSdrDeviceHandle, freq: u32){}
    fn set_bandwidth(&mut self, handle: &RtlSdrDeviceHandle, bw: u32, rate: u32){}
    fn get_if_freq(&self) -> u32 {0}
}