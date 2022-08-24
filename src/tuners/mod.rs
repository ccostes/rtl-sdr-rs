pub mod r820t;

use super::*;

pub const KNOWN_TUNERS: [TunerInfo; 1] = [r820t::TUNER_INFO];
pub enum Tuners {
    UNKNOWN,
    R820T(r820t::R820T),
}

impl Tuner for Tuners {
    fn init(&self, handle: &RtlSdrDeviceHandle) {
        match self {
            Tuners::R820T(r820t) => r820t.init(handle),
            Tuners::UNKNOWN => {},
        }
    }
    fn set_gain_mode(&mut self, handle: &RtlSdrDeviceHandle, mode: TunerGainMode) {
        match self {
            Tuners::R820T(r820t) => r820t.set_gain_mode(handle, mode),
            Tuners::UNKNOWN => {},
        }
    }
    fn set_freq(&mut self, handle: &RtlSdrDeviceHandle, freq: u32) {
        match self {
            Tuners::R820T(r820t) => r820t.set_freq(handle, freq),
            Tuners::R820T(r820t) => r820t.set_freq(handle, freq),
            Tuners::UNKNOWN => {},
        }
    }
    fn set_bandwidth(&mut self, handle: &RtlSdrDeviceHandle, bw: u32, rate: u32) {
        match self {
            Tuners::R820T(r820t) => r820t.set_bandwidth(handle, bw, rate),
            Tuners::UNKNOWN => {},
        }
    }
    fn get_if_freq(&self) -> u32 {
        match self {
            Tuners::R820T(r820t) => r820t.get_if_freq(),
            Tuners::UNKNOWN => { 0 },
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
    fn set_gain_mode(&mut self, handle: &RtlSdrDeviceHandle, mode: TunerGainMode);
    fn set_freq(&mut self, handle: &RtlSdrDeviceHandle, freq: u32);
    fn set_bandwidth(&mut self, handle: &RtlSdrDeviceHandle, bw: u32, rate: u32);
    fn get_if_freq(&self) -> u32;
}