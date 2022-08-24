
use rusb::{Context, Device, DeviceHandle, Result, UsbContext, Error};
mod usb;
use usb::RtlSdrDeviceHandle;

mod tuners;
use tuners::*;

pub (crate) const DEF_RTL_XTAL_FREQ: u32 = 28800000; // should this be here?

const VID: u16 = 0x0bda;
const PID: u16 = 0x2838;
const INTERFACE_ID: u8 = 0;

pub (crate) const FIR_LEN: usize = 16;
const DEFAULT_FIR: &'static [i32; FIR_LEN] = &[
    -54, -36, -41, -40, -32, -14, 14, 53,   // i8
    101, 156, 215, 273, 327, 372, 404, 421  // i12
];


pub enum TunerGainMode {
    AUTO,
    MANUAL(u32),
}

pub struct RtlSdr {
    handle: RtlSdrDeviceHandle,
    tuner: Tuners,
    freq: u32,                  // Hz
    rate: u32,                  // Hz
    bw: u32,
    direct_sampling: bool,
    xtal: u32,
    ppm_correction: u32,
    offset_freq: u32,
    corr: i32,                   // PPM
    force_bt: bool,
    fir: [i32; FIR_LEN],
}

impl RtlSdr {
    pub fn open() -> RtlSdr {
        let mut context = Context::new().unwrap();
        let (_device, handle) = 
            open_device(&mut context, VID, PID).expect("Failed to open USB device");
        
        let mut sdr = RtlSdr { 
            handle: RtlSdrDeviceHandle::new(handle),
            tuner: Tuners::UNKNOWN,
            freq: 0,
            rate: 0,
            bw: 0,
            ppm_correction: 0,
            xtal: 0,
            direct_sampling: false,
            offset_freq: 0,
            corr: 0,
            force_bt: false,
            fir: *DEFAULT_FIR,
        };
        sdr.init();
        sdr
    }

    pub fn set_tuner_gain_mode(&mut self, mode: TunerGainMode){
        self.set_i2c_repeater(true);
        self.tuner.set_gain_mode(&self.handle, mode);
        self.set_i2c_repeater(false);
    }

    // TODO: set_bias_tee

    pub fn reset_buffer(&self) {
        self.handle.write_reg(usb::BLOCK_USB, usb::USB_EPA_CTL, 0x1002, 2);
        self.handle.write_reg(usb::BLOCK_USB, usb::USB_EPA_CTL, 0x0000, 2);
    }

    pub fn set_if_freq(&self, freq: u32) {
        // Read corrected clock value
        let xtal = self.get_xtal_freq();
        let if_freq = (2_u32.pow(freq) / xtal) as i32 * -1;
        self.handle.demod_write_reg(1, 0x19, ((if_freq >> 16) & 0x3f) as u16, 1);
        self.handle.demod_write_reg(1, 0x1a, ((if_freq >> 8) & 0xff) as u16, 1);
        self.handle.demod_write_reg(1, 0x1b, (if_freq & 0xff) as u16, 1);
    }

    pub fn get_center_freq(&self) -> u32 {
        self.freq
    }

    pub fn set_center_freq(&mut self, freq: u32) {
        if self.direct_sampling {
            self.set_if_freq(freq);
        } else {
            self.set_i2c_repeater(true);
            // TODO: figure out offset_freq, currently never set
            self.tuner.set_freq(&self.handle, self.offset_freq);
            self.set_i2c_repeater(false);            
        }
        self.freq = freq;
    }

    pub fn get_sample_rate(&self) -> u32 {
        self.rate
    }

    pub fn set_sample_rate(&mut self, rate: u32) {
        // Check if rate is supported by the resampler
        if rate <= 225_000 || rate > 3_200_000 || (rate > 300000 && rate <= 900000) {
            println!("Invalid sample rate: {} Hz", rate);
            return ; // TODO: Err?
        }

        // Compute exact sample rate
        let rsamp_ratio = ((self.xtal * 2_u32.pow(22)) as u32 / rate) & 0x0ffffffc;
        let real_resamp_ratio = rsamp_ratio | ((rsamp_ratio & 0x08000000) << 1);
        let real_rate = (self.xtal * 2_u32.pow(22)) as f64 / real_resamp_ratio as f64;
        if rate as f64 != real_rate {
            println!("Exact sample rate is {} Hz", real_rate);
        }
        // Save exact rate
        self.rate = real_rate as u32;

        // Configure tuner
        self.set_i2c_repeater(true);
        let val = if self.bw > 0 {
            self.bw
        } else {
            self.rate
        };
        match self.tuner {
            Tuners::R820T(_) => {
                self.tuner.set_bandwidth(&self.handle, val, self.rate);
                self.set_if_freq(self.tuner.get_if_freq());
                self.set_center_freq(self.freq);
            },
            Tuners::UNKNOWN => {},
        }
        self.set_i2c_repeater(false);

        let mut tmp: u16 = (rsamp_ratio >> 16) as u16;
        self.handle.demod_write_reg(1, 0x9f, tmp, 2);
        tmp = (rsamp_ratio & 0xffff) as u16;
        self.handle.demod_write_reg(1, 0xa1, tmp, 2);

        self.set_sample_freq_correction(self.corr);
        
        // Reset demod (bit 3, soft_rst)
        self.handle.demod_write_reg(1, 0x01, 0x14, 1);
        self.handle.demod_write_reg(1, 0x01, 0x10, 1);

        // Recalculate offset frequency if offset tuning is enabled
        if self.offset_freq != 0 {
            // TODO: set_offset_tuning
        }
        
    }

    
    // RTL-SDR-BLOG Hack, enables us to turn on the bias tee by clicking on "offset tuning" 
    pub fn set_offset_tuning(&self, enable: bool) {
        // in software that doesn't have specified bias tee support.
        // Offset tuning is not used for R820T devices so it is no problem.
        self.set_gpio(0, enable);

        // TODO: implement the rest when we support tuners beyond R82xx
    }

    pub fn set_bias_tee(&self, on: bool) {
        self.set_gpio(0, on)
    }

    pub fn get_xtal_freq(&self) -> u32 {
        (self.xtal as f32 * (1.0 + self.ppm_correction as f32 / 1e6)) as u32
    }

    fn init(&mut self) {
        self.handle.print_device_info();
        self.handle.claim_interface(INTERFACE_ID);
        self.handle.test_write();
        self.power_on();
        self.set_i2c_repeater(true);
        
        self.tuner = {
            let tuner_id = match self.search_tuner() {
                Some(tid) => {
                    println!("Got tuner ID {}", tid);
                    tid
                }
                None => {
                    panic!("Failed to find tuner, aborting");
                }
            };
            match tuner_id {
                R820T_TUNER_ID => Tuners::R820T(r820t::R820T::new(&mut self.handle)),
                _ => panic!("Unable to find recognized tuner"),
            }
        };    
        // Finished Init
        self.set_i2c_repeater(false);
    }

    fn set_sample_freq_correction(&self, ppm: i32) {
        let offs = (ppm * (-1) * 2_i32.pow(24) / 1_000_000) as i16;
        self.handle.demod_write_reg(1, 0x3f, (offs & 0xff) as u16, 1);
        self.handle.demod_write_reg(1, 0x3e, ((offs >> 8) & 0x3f) as u16, 1);
    }

    fn set_gpio(&self, gpio_pin: u8, mut on: bool) {
        // If force_bt is on from the EEPROM, do not allow bias tee to turn off
        if self.force_bt {
            on = true;
        }
        self.set_gpio_output(gpio_pin);
        self.set_gpio_bit(gpio_pin, on);
    }

    fn set_gpio_bit(&self, mut gpio: u8, val: bool) {
        let mut r: u16 = 0;
        gpio = 1 << gpio;
        r = self.handle.read_reg(usb::BLOCK_SYS, usb::GPO, 1);
        r = if val {
            r | gpio as u16
        } else {
            r & !gpio as u16
        };
        self.handle.write_reg(usb::BLOCK_SYS, usb::GPO, r, 1);
    }

    fn set_gpio_output(&self, mut gpio: u8) {
        gpio = 1 << gpio;
        let mut r = 0;
        r = self.handle.read_reg(usb::BLOCK_SYS, usb::GPD, 1);
        self.handle.write_reg(usb::BLOCK_SYS, usb::GPD, r & !gpio as u16, 1);
        r = self.handle.read_reg(usb::BLOCK_SYS, usb::GPOE, 1);
        self.handle.write_reg(usb::BLOCK_SYS, usb::GPOE, r | gpio as u16, 1);
    }

    fn power_on(&self) {
        // Init baseband
        // println!("Initialize USB");
        self.handle.write_reg(usb::BLOCK_USB, usb::USB_SYSCTL, 0x09, 1);
        self.handle.write_reg(usb::BLOCK_USB, usb::USB_EPA_MAXPKT, 0x0002, 2);
        self.handle.write_reg(usb::BLOCK_USB, usb::USB_EPA_CTL, 0x1002, 2);

        // println!("Power-on demod");
        self.handle.write_reg(usb::BLOCK_SYS, usb::DEMOD_CTL_1, 0x22, 1);
        self.handle.write_reg(usb::BLOCK_SYS, usb::DEMOD_CTL, 0xe8, 1);

        // println!("Reset demod (bit 3, soft_rst)");
        self.handle.reset_demod();

        // println!("Disable spectrum inversion and adjust channel rejection");
        self.handle.demod_write_reg(1, 0x15, 0x00, 1);
        self.handle.demod_write_reg(1, 0x16, 0x00, 2);

        // println!("Clear DDC shift and IF registers");
        for i in 0..5 {
            self.handle.demod_write_reg(1, 0x16 + i, 0x00, 1);
        }
        self.set_fir(DEFAULT_FIR);

        // println!("Enable SDR mode, disable DAGC (bit 5)");
        self.handle.demod_write_reg(0, 0x19, 0x05, 1);

        // println!("Init FSM state-holding register");
        self.handle.demod_write_reg(1, 0x93, 0xf0, 1);
        self.handle.demod_write_reg(1, 0x94, 0x0f, 1);

        // Disable AGC (en_dagc, bit 0) (seems to have no effect)
        self.handle.demod_write_reg(1, 0x11, 0x00, 1);

        // Disable RF and IF AGC loop
        self.handle.demod_write_reg(1, 0x04, 0x00, 1);
        
        // Disable PID filter
        self.handle.demod_write_reg(0, 0x61, 0x60, 1);
        
        // opt_adc_iq = 0, default ADC_I/ADC_Q datapath
        self.handle.demod_write_reg(0, 0x06, 0x80, 1);
        
        // Enable Zero-IF mode, DC cancellation, and IQ estimation/compensation
        self.handle.demod_write_reg(1, 0xb1, 0x1b, 1);
        
        // Disable 4.096 MHz clock output on pin TP_CK0
        self.handle.demod_write_reg(0, 0x0d, 0x83, 1);
    }

    fn set_i2c_repeater(&self, enable: bool) {
        let val = match enable {
            true    => 0x18,
            false   => 0x10, 
        };
        self.handle.demod_write_reg(1, 0x01, val, 1);
    }


    pub fn set_fir(&self, fir: &[i32; FIR_LEN]) {
        const TMP_LEN: usize = 20;
        let mut tmp: [u8; TMP_LEN] = [0;TMP_LEN];

        for i in 0..7 {
            let val = fir[i];
            if val < -128 || val > 127 {
                panic!("i8 FIR coefficient out of bounds! {}", val);
            }
            tmp[i] = val as u8;
        }
        for i in (0..7).step_by(2) {
            let val0 = fir[8+i];
            let val1 = fir[8+i+1];
            if val0 < -2048 || val0 > 2047 {
                panic!("i12 FIR coefficient out of bounds: {}", val0)
            } else if val1 < -2048 || val1 > 2047 {
                panic!("i12 FIR coefficient out of bounds: {}", val1)
            }
            tmp[8 + i * 3 / 2] = (val0 >> 4) as u8;
            tmp[8 + i * 3 / 2 + 1] = ((val0 << 4) | ((val1 >> 8) & 0x0f)) as u8;
            tmp[8 + i * 3 / 2 + 2] = val1 as u8;
        }

        for i in 0..TMP_LEN - 1 {
            self.handle.demod_write_reg(1, 0x1c + 1, tmp[i] as u16, 1);
        }
    }

    fn search_tuner(&self) -> Option<&str> {
        for tuner_info in KNOWN_TUNERS.iter() {
            let regval = self.handle.i2c_read_reg(tuner_info.i2c_addr, tuner_info.check_addr);
            println!("Probing I2C address {:#02x} checking address {:#02x}", tuner_info.i2c_addr, tuner_info.check_addr);
            match regval {
                Ok(val) => {
                    // println!("Expecting value {:#02x}, got value {:#02x}", tuner_info.check_val, val);
                    if val == tuner_info.check_val {
                        return Some(tuner_info.id);
                    }
                }
                Err(e) => {
                    println!("Reading failed, continuing. Err: {}", e);
                }
            };
        }
        None
    }
}

fn open_device<T: UsbContext> (
    context: &mut T,
    vid: u16,
    pid: u16,
) -> Option<(Device<T>, DeviceHandle<T>)> {
    let devices = match context.devices() {
        Ok(d) => d,
        Err(_) => return None,
    };

    for device in devices.iter() {
        let device_desc = match device.device_descriptor() {
            Ok(d) => d,
            Err(_) => return None,
        };
        
        if device_desc.vendor_id() == vid && device_desc.product_id() == pid {
            match device.open() {
                Ok(handle) => return Some((device, handle)),
                Err(_) => continue,
            }
        }
    }
    None
}