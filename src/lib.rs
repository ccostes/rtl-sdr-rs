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
use error::{Result, RtlsdrError};
use rtlsdr::RtlSdr as Sdr;
use rusb::{Context, DeviceHandle, DeviceList, UsbContext};
use tuners::r82xx::{R820T_TUNER_ID, R828D_TUNER_ID};

pub struct TunerId;
impl TunerId {
    pub const R820T: &'static str = R820T_TUNER_ID;
    pub const R828D: &'static str = R828D_TUNER_ID;
}

pub const DEFAULT_BUF_LENGTH: usize = 16 * 16384;

pub struct DeviceDescriptors {
    list: DeviceList<Context>,
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
        let context = Context::new()?;
        let list = context.devices()?;
        Ok(Self { list })
    }

    /// Returns an iterator over the found RTL-SDR devices.
    pub fn iter(&self) -> impl Iterator<Item = DeviceDescriptor> + '_ {
        self.list
            .iter()
            .filter_map(|device| {
                let desc = device.device_descriptor().ok()?;
                device::is_known_device(desc.vendor_id(), desc.product_id()).then_some(device)
            })
            .enumerate()
            .filter_map(|(index, device)| {
                let desc = device.device_descriptor().ok()?;
                match device.open() {
                    Ok(handle) => {
                        let manufacturer = read_string(&handle, desc.manufacturer_string_index());
                        let product = read_string(&handle, desc.product_string_index());
                        let serial = read_string(&handle, desc.serial_number_string_index());

                        Some(DeviceDescriptor {
                            index,
                            vendor_id: desc.vendor_id(),
                            product_id: desc.product_id(),
                            manufacturer,
                            product,
                            serial,
                        })
                    }
                    Err(e) => {
                        log::warn!("Could not open device at index {}: {}", index, e);
                        None
                    }
                }
            })
    }
}

fn read_string<T: UsbContext>(handle: &DeviceHandle<T>, index: Option<u8>) -> String {
    index
        .and_then(|i| handle.read_string_descriptor_ascii(i).ok())
        .unwrap_or_default()
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum DeviceId<'a> {
    Index(usize),
    Serial(&'a str),
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
}
impl RtlSdr {
    pub fn open(device_id: DeviceId) -> Result<RtlSdr> {
        let dev = Device::new(device_id)?;
        let mut sdr = Sdr::new(dev);
        sdr.init()?;
        Ok(RtlSdr { sdr })
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
        self.sdr.deinit_baseband()
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
            .ok_or_else(|| {
                RtlsdrError::RtlsdrErr(format!("No device found at index {}", index))
            })
    }

    /// Get the serial number for a specific device by index
    pub fn get_device_serial(index: usize) -> Result<String> {
        Self::get_device_info(index).map(|info| info.serial)
    }
}
