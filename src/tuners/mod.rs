// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

pub mod r82xx;
use crate::device::Device;
use crate::error::Result;
use crate::TunerGain;

pub const KNOWN_TUNERS: [TunerInfo; 2] = [r82xx::R820T_TUNER_INFO, r82xx::R828D_TUNER_INFO];

#[derive(Debug, Clone, Copy)]

pub struct TunerInfo {
    pub id: &'static str,
    pub name: &'static str,
    pub i2c_addr: u8,
    pub check_addr: u8,
    pub check_val: u8,
    // pub gains: Vec<i8>,
}

/// Per-tuner backend interface. Implementations are stored as a
/// trait object inside [`crate::RtlSdr`], which is captured by
/// reference from the `read_async` callback. Since `read_async`
/// requires its closure to be `Send`, `&RtlSdr` must in turn be
/// `Send`, which requires `RtlSdr: Sync`, which requires
/// `Box<dyn Tuner>: Sync`. Bound the trait with `Send + Sync` so
/// any callback that wants to query the tuner mid-stream (e.g.
/// `read_gain` for a live AGC display) compiles. Existing
/// implementors (`NoTuner`, `R82xx`) hold only primitive / `Copy`
/// state so they already satisfy these auto-traits.
pub trait Tuner: std::fmt::Debug + Send + Sync {
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
