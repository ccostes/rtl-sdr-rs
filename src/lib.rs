// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! # rtlsdr Library
//! Library for interfacing with an RTL-SDR device.

mod async_read;
mod device;
pub mod error;
mod rtlsdr;
mod tuners;

pub use async_read::{
    AsyncReadConfigChange, AsyncReadControlHandle, AsyncReadEvent, AsyncReadHandle, CancelHandle,
};
use device::Device;
use error::{Result, RtlsdrError};
use nusb::MaybeFuture;
use rtlsdr::RtlSdr as Sdr;
use tuners::r82xx::{R820T_TUNER_ID, R828D_TUNER_ID};

pub struct TunerId;
impl TunerId {
    pub const R820T: &'static str = R820T_TUNER_ID;
    pub const R828D: &'static str = R828D_TUNER_ID;
}

pub const DEFAULT_BUF_LENGTH: usize = 16 * 16384;
pub const DEFAULT_ASYNC_BUF_NUMBER: usize = 15;

pub struct DeviceDescriptors {
    list: Vec<nusb::DeviceInfo>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceDescriptor {
    pub index: usize,
    pub vendor_id: u16,
    pub product_id: u16,
    pub manufacturer: String,
    pub product: String,
    pub serial: String,
}

impl DeviceDescriptors {
    pub fn new() -> Result<Self> {
        let list = nusb::list_devices()
            .wait()
            .map_err(RtlsdrError::from_usb)?
            .collect();
        Ok(Self { list })
    }

    /// Returns an iterator over the found RTL-SDR devices.
    pub fn iter(&self) -> impl Iterator<Item = DeviceDescriptor> + '_ {
        self.list
            .iter()
            .filter(|device| device::is_known_device(device.vendor_id(), device.product_id()))
            .enumerate()
            .map(|(index, device)| {
                let manufacturer = device.manufacturer_string().unwrap_or_default().to_string();
                let product = device.product_string().unwrap_or_default().to_string();
                let serial = device.serial_number().unwrap_or_default().to_string();

                DeviceDescriptor {
                    index,
                    vendor_id: device.vendor_id(),
                    product_id: device.product_id(),
                    manufacturer,
                    product,
                    serial,
                }
            })
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum DeviceId<'a> {
    Index(usize),
    Serial(&'a str),
    Fd(i32),
}

#[derive(Debug, Clone, PartialEq, Eq)]
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

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Sensor {
    TunerType,
    TunerGainDb,
    FrequencyCorrectionPpm,
}

#[derive(Debug, PartialEq)]
pub enum SensorValue {
    TunerType(String),
    TunerGainDb(i32),
    FrequencyCorrectionPpm(i32),
}

pub struct RtlSdr {
    sdr: Sdr,
    closed: bool,
}
impl RtlSdr {
    pub fn open(device_id: DeviceId) -> Result<RtlSdr> {
        let dev = Device::new(device_id)?;
        let mut sdr = Sdr::new(dev);
        sdr.init()?;
        Ok(RtlSdr { sdr, closed: false })
    }

    pub fn open_with_serial(serial: &str) -> Result<RtlSdr> {
        Self::open(DeviceId::Serial(serial))
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
        if self.closed {
            return Ok(());
        }
        self.closed = true;

        self.sdr.deinit_baseband()
    }
    pub fn reset_buffer(&self) -> Result<()> {
        self.sdr.reset_buffer()
    }
    pub fn read_sync(&self, buf: &mut [u8]) -> Result<usize> {
        self.sdr.read_sync(buf)
    }

    /// Continuously reads IQ samples and invokes `callback` on the calling thread.
    ///
    /// This method blocks until `cancel` is triggered or a transfer fails. Passing
    /// zero for `buf_num` or `buf_len` selects the corresponding crate default;
    /// nonzero buffer lengths must be multiples of 512 bytes. A `CancelHandle` is
    /// one-shot and cannot be reused for a later streaming session.
    pub fn read_async<F>(
        &self,
        buf_num: usize,
        buf_len: usize,
        cancel: &CancelHandle,
        callback: F,
    ) -> Result<()>
    where
        F: FnMut(&[u8]),
    {
        self.sdr.read_async(buf_num, buf_len, cancel, callback)
    }

    /// Starts an owned IQ stream with a cloneable live-control handle.
    ///
    /// The device is moved to a worker thread, which keeps multiple nusb bulk
    /// transfers queued while delivering [`AsyncReadEvent`] values through the
    /// returned handle. Configuration changes are acknowledged after the old
    /// transfers have been drained and the hardware has accepted the new
    /// setting. A reconfiguration marker is emitted before any new samples.
    pub fn into_async_reader(self, buf_num: usize, buf_len: usize) -> Result<AsyncReadHandle> {
        async_read::start_async_reader(self, buf_num, buf_len)
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
    pub fn read_tuner_gain(&self) -> Result<i32> {
        self.sdr.read_tuner_gain()
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
    pub fn get_tuner_id(&self) -> Result<&str> {
        self.sdr.get_tuner_id()
    }
    pub fn list_sensors(&self) -> Result<Vec<Sensor>> {
        Ok(vec![
            Sensor::TunerType,
            Sensor::TunerGainDb,
            Sensor::FrequencyCorrectionPpm,
        ])
    }
    pub fn read_sensor(&self, sensor: Sensor) -> Result<SensorValue> {
        match sensor {
            Sensor::TunerType => self
                .get_tuner_id()
                .map(|s| SensorValue::TunerType(s.to_string())),
            Sensor::TunerGainDb => self.sdr.read_tuner_gain().map(SensorValue::TunerGainDb),
            Sensor::FrequencyCorrectionPpm => Ok(SensorValue::FrequencyCorrectionPpm(
                self.get_freq_correction(),
            )),
        }
    }

    /// Get the number of available RTL-SDR devices
    pub fn get_device_count() -> Result<usize> {
        let descriptors = DeviceDescriptors::new()?;
        Ok(descriptors.iter().count())
    }

    /// List all available RTL-SDR devices
    pub fn list_devices() -> Result<Vec<DeviceDescriptor>> {
        let descriptors = DeviceDescriptors::new()?;
        Ok(descriptors.iter().collect())
    }

    /// Open the first available RTL-SDR device
    pub fn open_first_available() -> Result<RtlSdr> {
        let descriptors = DeviceDescriptors::new()?;
        let first_device = descriptors
            .iter()
            .next()
            .ok_or_else(|| RtlsdrError::RtlsdrErr("No RTL-SDR devices found".to_string()))?;
        Self::open_with_index(first_device.index)
    }

    /// Get device information for a specific device by index
    pub fn get_device_info(index: usize) -> Result<DeviceDescriptor> {
        let descriptors = DeviceDescriptors::new()?;
        let devices: Vec<DeviceDescriptor> = descriptors.iter().collect();
        devices
            .into_iter()
            .find(|d| d.index == index)
            .ok_or_else(|| RtlsdrError::RtlsdrErr(format!("No device found at index {}", index)))
    }

    /// Get the serial number for a specific device by index
    pub fn get_device_serial(index: usize) -> Result<String> {
        Self::get_device_info(index).map(|info| info.serial)
    }
}

impl Drop for RtlSdr {
    fn drop(&mut self) {
        if let Err(e) = self.close() {
            log::error!("Failed to power down on close: {e}");
        }
    }
}

#[cfg(test)]
mod public_api_tests {
    use super::{CancelHandle, Result, RtlSdr};

    #[allow(dead_code)]
    fn read_async_accepts_a_borrowed_callback(
        sdr: &RtlSdr,
        cancel: &CancelHandle,
        byte_count: &mut usize,
    ) -> Result<()> {
        sdr.read_async(1, 512, cancel, |buf| *byte_count += buf.len())
    }
}
