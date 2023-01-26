use super::{DirectSampleMode, TunerGain};
use crate::device::{
    Device, BLOCK_SYS, BLOCK_USB, DEMOD_CTL, DEMOD_CTL_1, EEPROM_SIZE, GPD, GPO, GPOE, USB_EPA_CTL,
    USB_EPA_MAXPKT, USB_SYSCTL,
};
use crate::error::Result;
use crate::error::RtlsdrError::RtlsdrErr;
use crate::tuners::r820t::{R820T, R82XX_IF_FREQ, TUNER_ID};
use crate::tuners::{NoTuner, Tuner, KNOWN_TUNERS};
use log::{error, info};

const INTERFACE_ID: u8 = 0;

const DEF_RTL_XTAL_FREQ: u32 = 28_800_000;
const MIN_RTL_XTAL_FREQ: u32 = DEF_RTL_XTAL_FREQ - 1000;
const MAX_RTL_XTAL_FREQ: u32 = DEF_RTL_XTAL_FREQ + 1000;

pub(crate) const FIR_LEN: usize = 16;
const DEFAULT_FIR: &'static [i32; FIR_LEN] = &[
    -54, -36, -41, -40, -32, -14, 14, 53, // i8
    101, 156, 215, 273, 327, 372, 404, 421, // i12
];

#[derive(Debug)]
pub struct RtlSdr {
    handle: Device,
    tuner: Box<dyn Tuner>,
    freq: u32, // Hz
    rate: u32, // Hz
    bw: u32,
    direct_sampling: DirectSampleMode,
    xtal: u32,
    tuner_xtal: u32,
    ppm_correction: u32,
    offset_freq: u32,
    corr: i32, // PPM
    force_bt: bool,
    force_ds: bool,
    fir: [i32; FIR_LEN],
}

impl RtlSdr {
    pub fn new(handle: Device) -> Self {
        RtlSdr {
            handle: handle,
            tuner: Box::new(NoTuner {}),
            freq: 0,
            rate: 0,
            bw: 0,
            ppm_correction: 0,
            xtal: DEF_RTL_XTAL_FREQ,
            tuner_xtal: DEF_RTL_XTAL_FREQ,
            direct_sampling: DirectSampleMode::Off,
            offset_freq: 0,
            corr: 0,
            force_bt: false,
            force_ds: false,
            fir: *DEFAULT_FIR,
        }
    }

    pub fn init(&mut self) -> Result<()> {
        self.handle.claim_interface(INTERFACE_ID)?;
        self.handle.test_write()?;
        self.init_baseband()?;
        self.set_i2c_repeater(true)?;

        self.tuner = {
            let tuner_id = match self.search_tuner() {
                Some(tid) => {
                    info!("Got tuner ID {}", tid);
                    tid
                }
                None => {
                    panic!("Failed to find tuner, aborting");
                }
            };
            match tuner_id {
                TUNER_ID => Box::new(R820T::new(&mut self.handle)),
                _ => panic!("Unable to find recognized tuner"),
            }
        };
        // Use the RTL clock value by default
        self.tuner_xtal = self.xtal;
        self.tuner.set_xtal_freq(self.get_tuner_xtal_freq())?;

        // disable Zero-IF mode
        self.handle.demod_write_reg(1, 0xb1, 0x1a, 1)?;

        // only enable In-phase ADC input
        self.handle.demod_write_reg(0, 0x08, 0x4d, 1)?;

        // the R82XX use 3.57 MHz IF for the DVB-T 6 MHz mode, and
        // 4.57 MHz for the 8 MHz mode
        self.set_if_freq(R82XX_IF_FREQ)?;

        // enable spectrum inversion
        self.handle.demod_write_reg(1, 0x15, 0x01, 1)?;

        // Hack to force the Bias T to always be on if we set the IR-Endpoint bit in the EEPROM to 0. Default on EEPROM is 1.
        let buf: [u8; EEPROM_SIZE] = [0; EEPROM_SIZE];
        self.handle.read_eeprom(&buf, 0, EEPROM_SIZE)?;
        if buf[7] & 0x02 != 0 {
            self.force_bt = false;
        } else {
            self.force_bt = true;
        }
        // Hack to force direct sampling mode to always be on if we set the remote-enabled bit in the EEPROM to 1. Default on EEPROM is 0.
        if buf[7] & 0x01 != 0 {
            self.force_ds = true;
        } else {
            self.force_ds = false;
        }
        // TODO: if(force_ds){tuner_type = TUNER_UNKNOWN}
        info!("Init tuner");
        self.tuner.init(&self.handle)?;

        // Finished Init
        self.set_i2c_repeater(false)?;
        info!("Init complete");
        Ok(())
    }

    pub fn get_tuner_gains(&self) -> Result<Vec<i32>> {
        self.tuner.get_gains()
    }

    // TunerGain has mode and gain, so this replaces rtlsdr_set_tuner_gain_mode
    pub fn set_tuner_gain(&mut self, gain: TunerGain) -> Result<()> {
        self.set_i2c_repeater(true)?;
        self.tuner.set_gain(&self.handle, gain)?;
        self.set_i2c_repeater(false)?;
        Ok(())
    }

    // TODO: set_bias_tee

    pub fn reset_buffer(&self) -> Result<()> {
        self.handle.write_reg(BLOCK_USB, USB_EPA_CTL, 0x1002, 2)?;
        self.handle.write_reg(BLOCK_USB, USB_EPA_CTL, 0x0000, 2)?;
        Ok(())
    }

    pub fn get_center_freq(&self) -> u32 {
        self.freq
    }

    pub fn set_center_freq(&mut self, freq: u32) -> Result<()> {
        if !matches!(self.direct_sampling, DirectSampleMode::Off) {
            self.set_if_freq(freq)?;
        } else {
            self.set_i2c_repeater(true)?;
            // TODO: figure out offset_freq, currently never set
            self.tuner.set_freq(&self.handle, freq - self.offset_freq)?;
            self.set_i2c_repeater(false)?;
        }
        self.freq = freq;
        Ok(())
    }

    pub fn set_if_freq(&self, freq: u32) -> Result<()> {
        // Get corrected clock value - start with default
        let rtl_xtal: u32 = DEF_RTL_XTAL_FREQ;
        // Apply PPM correction
        let base = 1u32 << 22;
        let if_freq: i32 = (freq as f64 * base as f64 / rtl_xtal as f64 * -1f64) as i32;

        let tmp = ((if_freq >> 16) as u16) & 0x3f;
        self.handle.demod_write_reg(1, 0x19, tmp, 1)?;
        let tmp = ((if_freq >> 8) as u16) & 0xff;
        self.handle.demod_write_reg(1, 0x1a, tmp, 1)?;
        let tmp = if_freq as u16 & 0xff;
        self.handle.demod_write_reg(1, 0x1b, tmp, 1)?;
        Ok(())
    }

    pub fn get_freq_correction(&self) -> i32 {
        self.corr
    }

    pub fn set_freq_correction(&mut self, ppm: i32) -> Result<()> {
        if self.corr == ppm {
            return Ok(());
        }
        self.corr = ppm;
        self.set_sample_freq_correction(ppm)?;

        // Read corrected clock value into tuner
        self.tuner.set_xtal_freq(self.get_tuner_xtal_freq())?;

        // Retune to apply new correction value
        self.set_center_freq(self.freq)?;
        Ok(())
    }

    pub fn get_sample_rate(&self) -> u32 {
        self.rate
    }

    pub fn set_sample_rate(&mut self, rate: u32) -> Result<()> {
        // Check if rate is supported by the resampler
        if rate <= 225_000 || rate > 3_200_000 || (rate > 300000 && rate <= 900000) {
            return Err(RtlsdrErr(format!("Invalid sample rate: {} Hz", rate)));
        }

        // Compute exact sample rate
        let rsamp_ratio =
            ((self.xtal as u128 * 2_u128.pow(22) / rate as u128) & 0x0ffffffc) as u128;
        info!(
            "set_sample_rate: rate: {}, xtal: {}, rsamp_ratio: {}",
            rate, self.xtal, rsamp_ratio
        );
        let real_resamp_ratio = rsamp_ratio | ((rsamp_ratio & 0x08000000) << 1);
        info!("real_resamp_ratio: {}", real_resamp_ratio);
        let real_rate = (self.xtal as u128 * 2_u128.pow(22)) as f64 / real_resamp_ratio as f64;
        if rate as f64 != real_rate {
            info!("Exact sample rate is {} Hz", real_rate);
        }
        // Save exact rate
        self.rate = real_rate as u32;

        // Configure tuner
        self.set_i2c_repeater(true)?;
        let val = if self.bw > 0 { self.bw } else { self.rate };
        self.tuner.set_bandwidth(&self.handle, val, self.rate)?;
        self.set_i2c_repeater(false)?;
        if self.tuner.get_info()?.id == TUNER_ID {
            self.set_if_freq(self.tuner.get_if_freq()?)?;
            self.set_center_freq(self.freq)?;
        }

        let mut tmp: u16 = (rsamp_ratio >> 16) as u16;
        self.handle.demod_write_reg(1, 0x9f, tmp, 2)?;
        tmp = (rsamp_ratio & 0xffff) as u16;
        self.handle.demod_write_reg(1, 0xa1, tmp, 2)?;

        self.set_sample_freq_correction(self.corr)?;

        // Reset demod (bit 3, soft_rst)
        self.handle.demod_write_reg(1, 0x01, 0x14, 1)?;
        self.handle.demod_write_reg(1, 0x01, 0x10, 1)?;

        // Recalculate offset frequency if offset tuning is enabled
        if self.offset_freq != 0 {
            self.set_offset_tuning(true)?;
        }
        Ok(())
    }

    pub fn set_tuner_bandwidth(&mut self, mut bw: u32) -> Result<()> {
        bw = if bw > 0 { bw } else { self.rate };
        self.set_i2c_repeater(true)?;
        self.tuner.set_bandwidth(&self.handle, bw, self.rate)?;
        self.set_i2c_repeater(false)?;
        if self.tuner.get_info()?.id == TUNER_ID {
            self.set_if_freq(self.tuner.get_if_freq()?)?;
            self.set_center_freq(self.freq)?;
        }
        self.bw = bw;
        Ok(())
    }

    pub fn set_testmode(&mut self, on: bool) -> Result<()> {
        match on {
            true => {
                self.handle.demod_write_reg(0, 0x19, 0x03, 1)?;
            }
            false => {
                self.handle.demod_write_reg(0, 0x19, 0x05, 1)?;
            }
        }
        Ok(())
    }

    pub fn set_direct_sampling(&mut self, mut mode: DirectSampleMode) -> Result<()> {
        if self.force_ds {
            mode = DirectSampleMode::OnSwap;
        }
        match mode {
            DirectSampleMode::On | DirectSampleMode::OnSwap => {
                self.set_i2c_repeater(true)?;
                self.tuner.exit(&self.handle)?;
                self.set_i2c_repeater(false)?;

                // Disable Zero-IF mode
                self.handle.demod_write_reg(1, 0xb1, 0x1a, 1)?;

                // Disable spectrum inversion
                self.handle.demod_write_reg(1, 0x15, 0x00, 1)?;

                // Only enable in-phase ADC input
                self.handle.demod_write_reg(0, 0x08, 0x4d, 1)?;

                // Check whether to swap I and Q ADC
                if matches!(mode, DirectSampleMode::OnSwap) {
                    self.handle.demod_write_reg(0, 0x06, 0x90, 1)?;
                    info!("Enabled direct sampling mode: ON (swapped)");
                } else {
                    self.handle.demod_write_reg(0, 0x06, 0x80, 1)?;
                    info!("Enabled direct sampling mode: ON");
                }
                self.direct_sampling = mode;
            }
            DirectSampleMode::Off => {
                self.set_i2c_repeater(true)?;
                self.tuner.init(&self.handle)?;
                self.set_i2c_repeater(false)?;

                if self.tuner.get_info()?.id == TUNER_ID {
                    // tuner init already does all this
                    // self.set_if_freq(R82XX_IF_FREQ);
                    // Enable spectrum inversion
                    // handle.demod_write_reg(1, 0x15, 0x01, 1);
                } else {
                    self.set_if_freq(0)?;

                    // Enable in-phase + Quadrature ADC input
                    self.handle.demod_write_reg(0, 0x08, 0xcd, 1)?;

                    // Enable Zero-IF mode
                    self.handle.demod_write_reg(1, 0xb1, 0x1b, 1)?;
                }
                // opt_adc_iq = 0, default ADC_I/ADC_Q datapath
                self.handle.demod_write_reg(0, 0x06, 0x80, 1)?;
                info!("Disabled direct sampling mode");
                self.direct_sampling = DirectSampleMode::Off;
            }
        }
        self.set_center_freq(self.freq)?;
        Ok(())
    }

    pub fn set_offset_tuning(&self, _enable: bool) -> Result<()> {
        // RTL-SDR-BLOG Hack, enables us to turn on the bias tee by clicking on "offset tuning"
        // in software that doesn't have specified bias tee support.
        // Offset tuning is not used for R820T devices so it is no problem.
        #[cfg(feature = "rtl_sdr_blog")]
        self.set_gpio(0, _enable)?;

        // TODO: implement the rest when we support tuners beyond R82xx
        Ok(())
    }

    pub fn set_bias_tee(&self, on: bool) -> Result<()> {
        Ok(self.set_gpio(0, on)?)
    }

    #[allow(dead_code)]
    pub fn get_xtal_freq(&self) -> u32 {
        (self.xtal as f32 * (1.0 + self.ppm_correction as f32 / 1e6)) as u32
    }

    pub fn get_tuner_xtal_freq(&self) -> u32 {
        (self.tuner_xtal as f32 * (1.0 + self.ppm_correction as f32 / 1e6)) as u32
    }

    #[allow(dead_code)]
    pub fn set_xtal_freq(&mut self, rtl_freq: u32, tuner_freq: u32) -> Result<()> {
        if rtl_freq > 0 && (rtl_freq < MIN_RTL_XTAL_FREQ || rtl_freq > MAX_RTL_XTAL_FREQ) {
            return Err(RtlsdrErr(format!(
                "set_xtal_freq error: rtl_freq {} out of bounds",
                rtl_freq
            )));
        }
        if rtl_freq > 0 && self.xtal != rtl_freq {
            self.xtal = rtl_freq;

            // Update xtal-dependent settings
            if self.rate != 0 {
                self.set_sample_rate(self.rate)?;
            }
        }

        if self.tuner.get_xtal_freq()? != tuner_freq {
            if tuner_freq == 0 {
                self.tuner_xtal = self.xtal;
            } else {
                self.tuner_xtal = tuner_freq;
            }

            // Read corrected clock value into tuner
            self.tuner.set_xtal_freq(self.get_tuner_xtal_freq())?;

            // Update xtal-dependent settings
            if self.freq != 0 {
                self.set_center_freq(self.freq)?;
            }
        }
        Ok(())
    }

    pub fn read_sync(&self, buf: &mut [u8]) -> Result<usize> {
        self.handle.bulk_transfer(buf)
    }

    fn init_baseband(&self) -> Result<()> {
        // Init baseband
        // info!("Initialize USB");
        self.handle.write_reg(BLOCK_USB, USB_SYSCTL, 0x09, 1)?;
        self.handle
            .write_reg(BLOCK_USB, USB_EPA_MAXPKT, 0x0002, 2)?;
        self.handle.write_reg(BLOCK_USB, USB_EPA_CTL, 0x1002, 2)?;

        // info!("Power-on demod");
        self.handle.write_reg(BLOCK_SYS, DEMOD_CTL_1, 0x22, 1)?;
        self.handle.write_reg(BLOCK_SYS, DEMOD_CTL, 0xe8, 1)?;

        // info!("Reset demod (bit 3, soft_rst)");
        self.handle.reset_demod()?;

        // info!("Disable spectrum inversion and adjust channel rejection");
        self.handle.demod_write_reg(1, 0x15, 0x00, 1)?;
        self.handle.demod_write_reg(1, 0x16, 0x00, 2)?;

        // info!("Clear DDC shift and IF registers");
        for i in 0..5 {
            self.handle.demod_write_reg(1, 0x16 + i, 0x00, 1)?;
        }
        self.set_fir(DEFAULT_FIR)?;

        // info!("Enable SDR mode, disable DAGC (bit 5)");
        self.handle.demod_write_reg(0, 0x19, 0x05, 1)?;

        // info!("Init FSM state-holding register");
        self.handle.demod_write_reg(1, 0x93, 0xf0, 1)?;
        self.handle.demod_write_reg(1, 0x94, 0x0f, 1)?;

        // Disable AGC (en_dagc, bit 0) (seems to have no effect)
        self.handle.demod_write_reg(1, 0x11, 0x00, 1)?;

        // Disable RF and IF AGC loop
        self.handle.demod_write_reg(1, 0x04, 0x00, 1)?;

        // Disable PID filter
        self.handle.demod_write_reg(0, 0x61, 0x60, 1)?;

        // opt_adc_iq = 0, default ADC_I/ADC_Q datapath
        self.handle.demod_write_reg(0, 0x06, 0x80, 1)?;

        // Enable Zero-IF mode, DC cancellation, and IQ estimation/compensation
        self.handle.demod_write_reg(1, 0xb1, 0x1b, 1)?;

        // Disable 4.096 MHz clock output on pin TP_CK0
        self.handle.demod_write_reg(0, 0x0d, 0x83, 1)?;

        Ok(())
    }

    pub fn deinit_baseband(&mut self) -> Result<()> {
        // Deinitialize tuner
        self.set_i2c_repeater(true)?;
        self.tuner.exit(&self.handle)?;
        self.set_i2c_repeater(false)?;

        // Power-off demodulator and ADCs
        self.handle.write_reg(BLOCK_SYS, DEMOD_CTL, 0x20, 1)?;
        Ok(())
    }

    fn set_sample_freq_correction(&self, ppm: i32) -> Result<()> {
        let offs = (ppm * (-1) * 2_i32.pow(24) / 1_000_000) as i16;
        self.handle
            .demod_write_reg(1, 0x3f, (offs & 0xff) as u16, 1)?;
        self.handle
            .demod_write_reg(1, 0x3e, ((offs >> 8) & 0x3f) as u16, 1)?;
        Ok(())
    }

    fn set_gpio(&self, gpio_pin: u8, mut on: bool) -> Result<()> {
        // If force_bt is on from the EEPROM, do not allow bias tee to turn off
        if self.force_bt {
            on = true;
        }
        self.set_gpio_output(gpio_pin)?;
        Ok(self.set_gpio_bit(gpio_pin, on)?)
    }

    fn set_gpio_bit(&self, mut gpio: u8, val: bool) -> Result<()> {
        gpio = 1 << gpio;
        let mut r = self.handle.read_reg(BLOCK_SYS, GPO, 1)?;
        r = if val {
            r | gpio as u16
        } else {
            r & !gpio as u16
        };
        self.handle.write_reg(BLOCK_SYS, GPO, r, 1)?;
        Ok(())
    }

    fn set_gpio_output(&self, mut gpio: u8) -> Result<()> {
        gpio = 1 << gpio;
        let mut r = self.handle.read_reg(BLOCK_SYS, GPD, 1)?;
        self.handle.write_reg(BLOCK_SYS, GPD, r & !gpio as u16, 1)?;
        r = self.handle.read_reg(BLOCK_SYS, GPOE, 1)?;
        self.handle.write_reg(BLOCK_SYS, GPOE, r | gpio as u16, 1)?;
        Ok(())
    }

    fn set_i2c_repeater(&self, enable: bool) -> Result<()> {
        let val = match enable {
            true => 0x18,
            false => 0x10,
        };
        self.handle
            .demod_write_reg(1, 0x01, val, 1)
            .and_then(|_| return Ok(()))
    }

    pub fn set_fir(&self, fir: &[i32; FIR_LEN]) -> Result<()> {
        const TMP_LEN: usize = 20;
        let mut tmp: [u8; TMP_LEN] = [0; TMP_LEN];
        // First 8 values are i8
        for i in 0..8 {
            let val = fir[i];
            if val < -128 || val > 127 {
                panic!("i8 FIR coefficient out of bounds! {}", val);
            }
            tmp[i] = val as u8;
        }
        // Next 12 are i12, so don't line up with byte boundaries and need to unpack
        // 12 i12 values from 4 pairs of bytes in fir. Example:
        // fir: 4b5, 7f8, 3e8, 619
        // tmp: 4b, 57, f8, 3e, 86, 19
        for i in (0..8).step_by(2) {
            let val0 = fir[8 + i];
            let val1 = fir[8 + i + 1];
            if val0 < -2048 || val0 > 2047 {
                panic!("i12 FIR coefficient out of bounds: {}", val0)
            } else if val1 < -2048 || val1 > 2047 {
                panic!("i12 FIR coefficient out of bounds: {}", val1)
            }
            tmp[8 + i * 3 / 2] = (val0 >> 4) as u8;
            tmp[8 + i * 3 / 2 + 1] = ((val0 << 4) | ((val1 >> 8) & 0x0f)) as u8;
            tmp[8 + i * 3 / 2 + 2] = val1 as u8;
        }

        for i in 0..TMP_LEN {
            self.handle
                .demod_write_reg(1, 0x1c + i as u16, tmp[i] as u16, 1)?;
        }
        Ok(())
    }

    fn search_tuner(&self) -> Option<&str> {
        for tuner_info in KNOWN_TUNERS.iter() {
            let regval = self
                .handle
                .i2c_read_reg(tuner_info.i2c_addr, tuner_info.check_addr);
            info!(
                "Probing I2C address {:#02x} checking address {:#02x}",
                tuner_info.i2c_addr, tuner_info.check_addr
            );
            match regval {
                Ok(val) => {
                    // info!("Expecting value {:#02x}, got value {:#02x}", tuner_info.check_val, val);
                    if val == tuner_info.check_val {
                        return Some(tuner_info.id);
                    }
                }
                Err(e) => {
                    error!("Reading failed, continuing. Err: {}", e);
                }
            };
        }
        None
    }
}
