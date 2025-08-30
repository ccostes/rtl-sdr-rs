// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! # rtlsdr Library
//! Library for interfacing with an RTL-SDR device.

mod device;
pub mod error;
mod rtlsdr;
mod tuners;

use device::Device;
use error::Result;
use rtlsdr::RtlSdr as Sdr;

pub const DEFAULT_BUF_LENGTH: usize = 16 * 16384;

#[derive(Debug, PartialEq)]
pub enum DeviceId {
    Index(usize),
    Fd(i32),
}

#[derive(Debug)]
pub enum TunerGain {
    Auto,
    Manual(i32),
}
#[derive(Debug)]
pub enum DirectSampleMode {
    Off,
    On,
    OnSwap, // Swap I and Q ADC, allowing to select between two inputs
}

pub struct RtlSdr {
    sdr: Sdr,
}
impl RtlSdr {
    pub fn open(device_id: DeviceId) -> Result<RtlSdr> {
        let dev = Device::new(device_id)?;
        let mut sdr = Sdr::new(dev);
        sdr.init()?;
        Ok(RtlSdr { sdr: sdr })
    }
    
    /// Convenience function to open device by index (backward compatibility)
    pub fn open_with_index(index: usize) -> Result<RtlSdr> {
        Self::open(DeviceId::Index(index))
    }
    
    /// Convenience function to open device by file descriptor  
    pub fn open_with_fd(fd: i32) -> Result<RtlSdr> {
        Self::open(DeviceId::Fd(fd))
    }
    pub fn close(&mut self) -> Result<()> {
        // TODO: wait until async is inactive
        Ok(self.sdr.deinit_baseband()?)
    }
    pub fn reset_buffer(&self) -> Result<()> {
        self.sdr.reset_buffer()
    }
    pub fn read_sync(&self, buf: &mut [u8]) -> Result<usize> {
        self.sdr.read_sync(buf)
    }
    pub fn get_center_freq(&self) -> u32 {
        self.sdr.get_center_freq()
    }
    pub fn set_center_freq(&mut self, freq: u32) -> Result<()> {
        self.sdr.set_center_freq(freq)
    }
    pub fn get_tuner_gains(&self) -> Result<Vec<i32>> {
        self.sdr.get_tuner_gains()
    }
    pub fn set_tuner_gain(&mut self, gain: TunerGain) -> Result<()> {
        self.sdr.set_tuner_gain(gain)
    }
    pub fn get_freq_correction(&self) -> i32 {
        self.sdr.get_freq_correction()
    }
    pub fn set_freq_correction(&mut self, ppm: i32) -> Result<()> {
        self.sdr.set_freq_correction(ppm)
    }
    pub fn get_sample_rate(&self) -> u32 {
        self.sdr.get_sample_rate()
    }
    pub fn set_sample_rate(&mut self, rate: u32) -> Result<()> {
        self.sdr.set_sample_rate(rate)
    }
    pub fn set_tuner_bandwidth(&mut self, bw: u32) -> Result<()> {
        self.sdr.set_tuner_bandwidth(bw)
    }
    pub fn set_testmode(&mut self, on: bool) -> Result<()> {
        self.sdr.set_testmode(on)
    }
    pub fn set_direct_sampling(&mut self, mode: DirectSampleMode) -> Result<()> {
        self.sdr.set_direct_sampling(mode)
    }
    pub fn set_bias_tee(&self, on: bool) -> Result<()> {
        self.sdr.set_bias_tee(on)
    }
}
