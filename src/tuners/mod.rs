pub mod r820t;

use super::*;

pub const KNOWN_TUNERS: [TunerInfo; 1] = [r820t::TUNER_INFO];
pub enum Tuners {
    R820T(r820t::R820T),
}

impl Tuner for Tuners {
    fn init(&self, handle: &RtlSdrDeviceHandle) {
        match self {
            Tuners::R820T(r820t) => r820t.init(handle)
        }
    }
    fn set_gain_mode(&self, mode: TunerGainMode) {
        match self {
            Tuners::R820T(r820t) => r820t.set_gain_mode(mode)
        }
    }
}

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
    fn set_gain_mode(&self, mode: TunerGainMode);
}