use super::{Tuner, TunerGain, TunerInfo};
use crate::device::Device;
use crate::error::Result;
use crate::error::RtlsdrError::RtlsdrErr;
use log::info;

const R820T_I2C_ADDR: u16 = 0x34;
// const R828D_I2C_ADDR: u8 = 0x74; for now only support the T
const VER_NUM: u8 = 49;
pub const R82XX_IF_FREQ: u32 = 3570000;
const NUM_REGS: usize = 32;
const RW_REG_START: usize = 5; // registers 0-4 are read-only
const NUM_CACHE_REGS: usize = NUM_REGS - RW_REG_START; // only cache RW regs
const MAX_I2C_MSG_LEN: usize = 8;

// Init registers (32 total, first 5 are read-only)
const REG_INIT: [u8; NUM_CACHE_REGS] = [
    0x83, 0x32, 0x75, /* 05 to 07 */
    0xc0, 0x40, 0xd6, 0x6c, /* 08 to 0b */
    0xf5, 0x63, 0x75, 0x68, /* 0c to 0f */
    0x6c, 0x83, 0x80, 0x00, /* 10 to 13 */
    0x0f, 0x00, 0xc0, 0x30, /* 14 to 17 */
    0x48, 0xcc, 0x60, 0x00, /* 18 to 1b */
    0x54, 0xae, 0x4a, 0xc0, /* 1c to 1f */
];

/* measured with a Racal 6103E GSM test set at 928 MHz with -60 dBm
* input power, for raw results see:
* http://steve-m.de/projects/rtl-sdr/gain_measurement/r820t/
*/
const _VGA_BASE_GAIN: i32 = -47;
const GAINS: [i32; 29] = [
    0, 9, 14, 27, 37, 77, 87, 125, 144, 157, 166, 197, 207, 229, 254, 280, 297, 328, 338, 364, 372,
    386, 402, 421, 434, 439, 445, 480, 496,
];
const _R82XX_VGA_GAIN_STEPS: [i32; 16] = [
    0, 26, 26, 30, 42, 35, 24, 13, 14, 32, 36, 34, 35, 37, 35, 36,
];

const R82XX_LNA_GAIN_STEPS: [i32; 16] =
    [0, 9, 13, 40, 38, 13, 31, 22, 26, 31, 26, 14, 19, 5, 35, 13];

const R82XX_MIXER_GAIN_STEPS: [i32; 16] =
    [0, 5, 10, 10, 19, 9, 10, 25, 17, 10, 8, 16, 13, 6, 3, -8];

struct FreqRange {
    freq: u32,       // Start freq, in MHz
    open_d: u8,      // low
    rf_mux_ploy: u8, // R26[7:6]=0 (LPF)  R26[1:0]=2 (low)
    tf_c: u8,        // R27[7:0]  band2,band0
    xtal_cap20p: u8, // R16[1:0]  20pF (10)
    xtal_cap10p: u8,
    xtal_cap0p: u8,
}

const FREQ_RANGES: [FreqRange; 21] = [
    FreqRange {
        freq: 0,
        open_d: 0x08,
        rf_mux_ploy: 0x02,
        tf_c: 0xdf,
        xtal_cap20p: 0x02,
        xtal_cap10p: 0x01,
        xtal_cap0p: 0x00,
    },
    FreqRange {
        freq: 50,
        open_d: 0x08,
        rf_mux_ploy: 0x02,
        tf_c: 0xbe,
        xtal_cap20p: 0x02,
        xtal_cap10p: 0x01,
        xtal_cap0p: 0x00,
    },
    FreqRange {
        freq: 55,
        open_d: 0x08,
        rf_mux_ploy: 0x02,
        tf_c: 0x8b,
        xtal_cap20p: 0x02,
        xtal_cap10p: 0x01,
        xtal_cap0p: 0x00,
    },
    FreqRange {
        freq: 60,
        open_d: 0x08,
        rf_mux_ploy: 0x02,
        tf_c: 0x7b,
        xtal_cap20p: 0x02,
        xtal_cap10p: 0x01,
        xtal_cap0p: 0x00,
    },
    FreqRange {
        freq: 65,
        open_d: 0x08,
        rf_mux_ploy: 0x02,
        tf_c: 0x69,
        xtal_cap20p: 0x02,
        xtal_cap10p: 0x01,
        xtal_cap0p: 0x00,
    },
    FreqRange {
        freq: 70,
        open_d: 0x08,
        rf_mux_ploy: 0x02,
        tf_c: 0x58,
        xtal_cap20p: 0x02,
        xtal_cap10p: 0x01,
        xtal_cap0p: 0x00,
    },
    FreqRange {
        freq: 75,
        open_d: 0x00,
        rf_mux_ploy: 0x02,
        tf_c: 0x44,
        xtal_cap20p: 0x02,
        xtal_cap10p: 0x01,
        xtal_cap0p: 0x00,
    },
    FreqRange {
        freq: 80,
        open_d: 0x00,
        rf_mux_ploy: 0x02,
        tf_c: 0x44,
        xtal_cap20p: 0x02,
        xtal_cap10p: 0x01,
        xtal_cap0p: 0x00,
    },
    FreqRange {
        freq: 90,
        open_d: 0x00,
        rf_mux_ploy: 0x02,
        tf_c: 0x34,
        xtal_cap20p: 0x01,
        xtal_cap10p: 0x01,
        xtal_cap0p: 0x00,
    },
    FreqRange {
        freq: 100,
        open_d: 0x00,
        rf_mux_ploy: 0x02,
        tf_c: 0x34,
        xtal_cap20p: 0x01,
        xtal_cap10p: 0x01,
        xtal_cap0p: 0x00,
    },
    FreqRange {
        freq: 110,
        open_d: 0x00,
        rf_mux_ploy: 0x02,
        tf_c: 0x24,
        xtal_cap20p: 0x01,
        xtal_cap10p: 0x01,
        xtal_cap0p: 0x00,
    },
    FreqRange {
        freq: 120,
        open_d: 0x00,
        rf_mux_ploy: 0x02,
        tf_c: 0x24,
        xtal_cap20p: 0x01,
        xtal_cap10p: 0x01,
        xtal_cap0p: 0x00,
    },
    FreqRange {
        freq: 140,
        open_d: 0x00,
        rf_mux_ploy: 0x02,
        tf_c: 0x14,
        xtal_cap20p: 0x01,
        xtal_cap10p: 0x01,
        xtal_cap0p: 0x00,
    },
    FreqRange {
        freq: 180,
        open_d: 0x00,
        rf_mux_ploy: 0x02,
        tf_c: 0x13,
        xtal_cap20p: 0x00,
        xtal_cap10p: 0x00,
        xtal_cap0p: 0x00,
    },
    FreqRange {
        freq: 220,
        open_d: 0x00,
        rf_mux_ploy: 0x02,
        tf_c: 0x13,
        xtal_cap20p: 0x00,
        xtal_cap10p: 0x00,
        xtal_cap0p: 0x00,
    },
    FreqRange {
        freq: 250,
        open_d: 0x00,
        rf_mux_ploy: 0x02,
        tf_c: 0x11,
        xtal_cap20p: 0x00,
        xtal_cap10p: 0x00,
        xtal_cap0p: 0x00,
    },
    FreqRange {
        freq: 280,
        open_d: 0x00,
        rf_mux_ploy: 0x02,
        tf_c: 0x00,
        xtal_cap20p: 0x00,
        xtal_cap10p: 0x00,
        xtal_cap0p: 0x00,
    },
    FreqRange {
        freq: 310,
        open_d: 0x00,
        rf_mux_ploy: 0x41,
        tf_c: 0x00,
        xtal_cap20p: 0x00,
        xtal_cap10p: 0x00,
        xtal_cap0p: 0x00,
    },
    FreqRange {
        freq: 450,
        open_d: 0x00,
        rf_mux_ploy: 0x41,
        tf_c: 0x00,
        xtal_cap20p: 0x00,
        xtal_cap10p: 0x00,
        xtal_cap0p: 0x00,
    },
    FreqRange {
        freq: 588,
        open_d: 0x00,
        rf_mux_ploy: 0x40,
        tf_c: 0x00,
        xtal_cap20p: 0x00,
        xtal_cap10p: 0x00,
        xtal_cap0p: 0x00,
    },
    FreqRange {
        freq: 650,
        open_d: 0x00,
        rf_mux_ploy: 0x40,
        tf_c: 0x00,
        xtal_cap20p: 0x00,
        xtal_cap10p: 0x00,
        xtal_cap0p: 0x00,
    },
];

#[allow(dead_code)]
enum TunerType {
    TunerRadio,
    TunerAnalogTv,
    TunerDigitalTv,
}

#[derive(Debug)]
#[allow(dead_code)]
enum XtalCapValue {
    XtalLowCap30p,
    XtalLowCap20p,
    XtalLowCap10p,
    XtalLowCap0p,
    XtalHighCap0p,
}

#[allow(dead_code)]
const XTAL_CAPACITOR_VALUES: [u8; 5] = [
    0x0b, // XTAL_LOW_CAP_30P
    0x02, // XTAL_LOW_CAP_20P
    0x01, // XTAL_LOW_CAP_10P
    0x00, // XTAL_LOW_CAP_0P
    0x10, // XTAL_HIGH_CAP_0P
];

#[allow(dead_code)]
enum DeliverySystem {
    SysUndefined,
    SysDvbt,
    SysDvbt2,
    SysIsdbt,
}

#[derive(Debug)]
pub struct R820T {
    pub info: TunerInfo,
    regs: [u8; NUM_CACHE_REGS],
    pub freq: u32,
    int_freq: u32,
    xtal_cap_sel: XtalCapValue,
    xtal: u32,
    use_predetect: bool,
    has_lock: bool,
    fil_cal_code: u8,
    init_done: bool,
}

pub const TUNER_ID: &str = "r820t";

pub const TUNER_INFO: TunerInfo = TunerInfo {
    id: TUNER_ID,
    name: "Rafael Micro R820T",
    i2c_addr: 0x34,
    check_addr: 0x00,
    check_val: 0x69,
    // gains: vec![
    //     0, 9, 14, 27, 37, 77, 87, 125, 144, 157, 166, 197, 207, 229, 254, 280, 297, 328, 338, 364,
    //     372, 386, 402, 421, 434, 439, 445, 480, 496,
    // ],
};

impl R820T {
    pub fn new(_handle: &mut Device) -> R820T {
        let tuner = R820T {
            info: TUNER_INFO,
            regs: REG_INIT,
            freq: 0,
            int_freq: 0,
            xtal_cap_sel: XtalCapValue::XtalLowCap30p,
            xtal: 0,
            has_lock: false,
            init_done: false,
            use_predetect: false,
            fil_cal_code: 0,
        };
        tuner
    }
}

impl Tuner for R820T {
    // Combined from r820t_init and r82xx_init
    fn init(&mut self, handle: &Device) -> Result<()> {
        // TODO: set different I2C address and rafael_chip for R828D
        self.use_predetect = false;

        // <original>TODO: R828D might need r82xx_xtal_check()
        self.xtal_cap_sel = XtalCapValue::XtalHighCap0p;

        // Initialize registers
        self.write_regs(handle, 0x05, &REG_INIT)?;

        self.set_tv_standard(handle, 3, TunerType::TunerDigitalTv)?;
        self.sysfreq_sel(
            handle,
            0,
            TunerType::TunerDigitalTv,
            DeliverySystem::SysDvbt,
        )?;
        self.init_done = true;
        Ok(())
    }

    fn get_info(&self) -> Result<TunerInfo> {
        Ok(self.info)
    }

    fn get_gains(&self) -> Result<Vec<i32>> {
        Ok(GAINS.to_vec())
    }

    fn read_gain(&self, handle: &Device) -> Result<i32> {
        let mut data: [u8; 4] = [0; 4];
        self.read_reg(handle, 0x00, &mut data, 4)?;
        let gain = ((data[3] & 0x0f) << 1) + ((data[3] & 0xf0) >> 4);
        Ok(gain as i32)
    }

    fn set_gain(&mut self, handle: &Device, mode: TunerGain) -> Result<()> {
        match mode {
            TunerGain::Auto => {
                // LNA
                self.write_reg_mask(handle, 0x05, 0, 0x10)?;
                // Mixer
                self.write_reg_mask(handle, 0x07, 0x10, 0x10)?;
                // Set fixed VGA gain for now (26.5 dB)
                self.write_reg_mask(handle, 0x0c, 0x0b, 0x9f)?;
            }
            TunerGain::Manual(gain) => {
                let mut data: [u8; 4] = [0; 4];
                // LNA auto off
                self.write_reg_mask(handle, 0x05, 0x10, 0x10)?;
                // Mixer auto off
                self.write_reg_mask(handle, 0x07, 0, 0x10)?;

                self.read_reg(handle, 0x00, &mut data, 4)?;

                // Set fixed VGA gain for now (16.3 dB)
                self.write_reg_mask(handle, 0x0c, 0x08, 0x9f)?; //init val 0x08 0x0c works well at 1.7

                let mut total_gain: i32 = 0;
                let mut mix_index: u8 = 0;
                let mut lna_index: u8 = 0;
                for _ in 0..15 {
                    if total_gain >= gain {
                        break;
                    }
                    lna_index += 1;
                    total_gain += R82XX_LNA_GAIN_STEPS[lna_index as usize];

                    if total_gain >= gain {
                        break;
                    }

                    mix_index += 1;
                    total_gain += R82XX_MIXER_GAIN_STEPS[mix_index as usize];
                }
                // Set LNA gain
                self.write_reg_mask(handle, 0x05, lna_index, 0x0f)?;

                // Set mixer gain
                self.write_reg_mask(handle, 0x07, mix_index, 0x0f)?;

                // LNA
                self.write_reg_mask(handle, 0x05, 0, 0x10)?;

                // Mixer
                self.write_reg_mask(handle, 0x07, 0x10, 0x10)?;

                // Set fixed VGA gain for now (26.5dB)
                self.write_reg_mask(handle, 0x0c, 0x0b, 0x9f)?;
            }
        }
        Ok(())
    }

    fn set_freq(&mut self, handle: &Device, freq: u32) -> Result<()> {
        info!("set_freq - freq: {}", freq);
        let lo_freq = freq + self.int_freq;
        info!("set_freq - lo_freq: {}", lo_freq);
        self.set_mux(handle, lo_freq)?;
        self.set_pll(handle, lo_freq)?;

        // TODO: Some extra stuff for the 828D tuner when we support that
        Ok(())
    }

    fn set_bandwidth(&mut self, handle: &Device, bw_in: u32, _rate: u32) -> Result<()> {
        let mut bw: i32 = bw_in as i32;
        const FILT_HP_BW1: i32 = 350_000;
        const FILT_HP_BW2: i32 = 380_000;
        const R82XX_IF_LOW_PASS_BW_TABLE: [i32; 10] = [
            1_700_000, 1_600_000, 1_550_000, 1_450_000, 1_200_000, 900_000, 700_000, 550_000,
            450_000, 350_000,
        ];

        let (reg_0a, reg_0b): (u8, u8) = if bw > 7_000_000 {
            // BW: 8MHz
            self.int_freq = 4_570_000;
            (0x10, 0x0b)
        } else if bw > 6_000_000 {
            // BW: 7MHz
            self.int_freq = 4_570_000;
            (0x10, 0x2a)
        } else if bw > R82XX_IF_LOW_PASS_BW_TABLE[0] + FILT_HP_BW1 + FILT_HP_BW2 {
            // BW: 6MHz
            self.int_freq = 3_570_000;
            (0x10, 0x6b)
        } else {
            self.int_freq = 2_300_000;
            let (reg_0a, mut reg_0b): (u8, u8) = (0x00, 0x80);
            let mut real_bw = 0;

            if bw > R82XX_IF_LOW_PASS_BW_TABLE[0] + FILT_HP_BW1 {
                bw -= FILT_HP_BW2;
                self.int_freq += FILT_HP_BW2 as u32;
                real_bw += FILT_HP_BW2;
            } else {
                reg_0b |= 0x20;
            }

            if bw > R82XX_IF_LOW_PASS_BW_TABLE[0] {
                bw -= FILT_HP_BW1;
                self.int_freq += FILT_HP_BW1 as u32;
                real_bw += FILT_HP_BW1;
            } else {
                reg_0b |= 0x40;
            }

            // Find low-pass filter
            let mut lp_idx = 0;
            // Want the element before the first that is lower than bw
            for (i, freq) in R82XX_IF_LOW_PASS_BW_TABLE.iter().enumerate() {
                if bw > *freq {
                    break;
                }
                lp_idx = i;
            }
            reg_0b |= 15 - lp_idx as u8;
            real_bw += R82XX_IF_LOW_PASS_BW_TABLE[lp_idx];

            self.int_freq -= (real_bw / 2) as u32;
            (reg_0a, reg_0b)
        };

        self.write_reg_mask(handle, 0x0a, reg_0a, 0x10)?;
        self.write_reg_mask(handle, 0x0b, reg_0b, 0xef)?;
        Ok(())
    }

    fn get_if_freq(&self) -> Result<u32> {
        Ok(self.int_freq)
    }

    fn get_xtal_freq(&self) -> Result<u32> {
        Ok(self.xtal)
    }

    fn set_xtal_freq(&mut self, freq: u32) -> Result<()> {
        self.xtal = freq;
        Ok(())
    }

    fn exit(&mut self, handle: &Device) -> Result<()> {
        // If device was not initialized yet don't need to standby
        if !self.init_done {
            return Ok(());
        }
        self.write_regs(handle, 0x06, &[0xb1])?;
        self.write_regs(handle, 0x05, &[0xa0])?;
        self.write_regs(handle, 0x07, &[0x3a])?;
        self.write_regs(handle, 0x08, &[0x40])?;
        self.write_regs(handle, 0x09, &[0xc0])?;
        self.write_regs(handle, 0x0a, &[0x36])?;
        self.write_regs(handle, 0x0c, &[0x35])?;
        self.write_regs(handle, 0x0f, &[0x68])?;
        self.write_regs(handle, 0x11, &[0x03])?;
        self.write_regs(handle, 0x17, &[0xf4])?;
        self.write_regs(handle, 0x19, &[0x0c])?;
        Ok(())
    }
}

impl R820T {
    // Tuning logic

    fn set_mux(&mut self, handle: &Device, freq: u32) -> Result<()> {
        // Get the proper frequency range
        let freq_mhz = freq / 1_000_000;
        // Find the range that freq is within
        let range = {
            let mut r: &FreqRange = &FREQ_RANGES[0];
            for range in FREQ_RANGES.iter() {
                if freq_mhz < range.freq {
                    // past freq, break
                    break;
                }
                // range still below freq, save it and continue iterating
                r = range;
            }
            r
        };

        // Open Drain
        self.write_reg_mask(handle, 0x17, range.open_d, 0x08)?;

        // RF_MUX, Polymux
        self.write_reg_mask(handle, 0x1a, range.rf_mux_ploy, 0xc3)?;

        // TF Band
        self.write_regs(handle, 0x1b, &[range.tf_c])?;

        // XTAL CAP & Drive
        let val = match self.xtal_cap_sel {
            XtalCapValue::XtalLowCap30p | XtalCapValue::XtalLowCap20p => range.xtal_cap20p | 0x08,
            XtalCapValue::XtalLowCap10p => range.xtal_cap10p | 0x08,
            XtalCapValue::XtalHighCap0p => range.xtal_cap0p | 0x00,
            XtalCapValue::XtalLowCap0p => range.xtal_cap0p | 0x08,
        };
        self.write_reg_mask(handle, 0x10, val, 0x0b)?;
        self.write_reg_mask(handle, 0x08, 0x00, 0x3f)?;
        self.write_reg_mask(handle, 0x09, 0x00, 0x3f)?;
        Ok(())
    }

    fn set_pll(&mut self, handle: &Device, freq: u32) -> Result<()> {
        // Frequency in kHz
        let freq_khz = (freq + 500) / 1000;
        info!("freq (kHz): {}", freq_khz);
        let pll_ref = self.xtal;
        let pll_ref_khz = (self.xtal + 500) / 1000;

        let refdiv2 = 0;
        self.write_reg_mask(handle, 0x10, refdiv2, 0x10)?;

        // Set PLL auto-tune = 128kHz
        self.write_reg_mask(handle, 0x1a, 0x00, 0x0c)?;

        // Set VCO current = 100 (RTL-SDR Blog Mod: MAX CURRENT)
        #[cfg(feature = "rtl_sdr_blog")]
        self.write_reg_mask(handle, 0x12, 0x06, 0xff)?;
        #[cfg(not(feature = "rtl_sdr_blog"))]
        self.write_reg_mask(handle, 0x12, 0x80, 0xe0)?;

        // Test turning tracking filter off
        // self.write_reg_mask(handle, 0x1a, 0x40, 0xc0);

        // Calculate divider
        let vco_min: u32 = 1770000;
        let vco_max: u32 = vco_min * 2;
        let mut mix_div: u8 = 2;
        let mut div_num: u8 = 0;
        while mix_div <= 64 {
            if ((freq_khz * mix_div as u32) >= vco_min) && ((freq_khz * mix_div as u32) < vco_max) {
                let mut div_buf = mix_div;
                while div_buf > 2 {
                    div_buf = div_buf >> 1;
                    div_num += 1;
                }
                break;
            }
            mix_div = mix_div << 1;
        }

        let mut data: [u8; 5] = [0; 5];
        self.read_reg(handle, 0x00, &mut data, 5)?;
        // TODO: if chip is R828D set vco_power_ref = 1
        let vco_power_ref = 2;
        let vco_fine_tune = (data[4] & 0x30) >> 4;
        if vco_fine_tune > vco_power_ref {
            div_num = div_num - 1;
        } else if vco_fine_tune < vco_power_ref {
            div_num = div_num + 1;
        }
        self.write_reg_mask(handle, 0x10, div_num << 5, 0xe0)?;

        let vco_freq = freq as u64 * mix_div as u64;
        info!("vco_freq: {}", vco_freq);
        let nint = (vco_freq / (2 * pll_ref as u64)) as u8;
        info!("nint: {}", nint);
        // VCO contribution by SDM (kHz)
        let mut vco_fra = ((vco_freq - 2 * pll_ref as u64 * nint as u64) / 1000) as u32;

        if nint > ((128 / vco_power_ref) - 1) {
            return Err(RtlsdrErr(format!(
                "[R82xx] No valid PLL values for {} Hz!",
                freq
            )));
        }
        // Nint = 4 * Ni2c + Si2c + 13
        // Some weird wrap-around stuff here, example cases from original code:
        // nint: 31 ni: 4   si: 2
        // nint: 3  ni: 254 si: 254
        let ni = ((nint as i32).overflowing_sub(13).0 / 4) as u8;
        let si = (nint as i32 - 4 * ni as i32 - 13) as u8;
        info!(
            "ni: {}, si: {}, reg: {}",
            ni,
            si,
            ni.overflowing_add(si << 6).0
        );
        self.write_regs(handle, 0x14, &[ni.overflowing_add(si << 6).0])?;

        // pw_sdm
        if vco_fra == 0 {
            self.write_reg_mask(handle, 0x12, 0x08, 0x08)?;
        } else {
            self.write_reg_mask(handle, 0x12, 0x00, 0x08)?;
        }

        // SDM Calculator
        let mut sdm = 0;
        let mut n_sdm = 2;
        while vco_fra > 1 {
            if vco_fra > (2 * pll_ref_khz / n_sdm) {
                sdm = sdm + 32768 / (n_sdm / 2);
                vco_fra = vco_fra - 2 * pll_ref_khz / n_sdm;
                if n_sdm >= 0x8000 {
                    break;
                }
            }
            n_sdm = n_sdm << 1;
        }
        self.write_regs(handle, 0x16, &[(sdm >> 8) as u8])?;
        self.write_regs(handle, 0x15, &[(sdm & 0xff) as u8])?;
        for i in 0..2 {
            // Check if PLL has locked
            self.read_reg(handle, 0x00, &mut data, 3)?;
            if data[2] & 0x40 != 0 {
                break;
            }
            if i == 0 {
                // Didn't lock, increase VCO current
                #[cfg(feature = "rtl_sdr_blog")]
                self.write_reg_mask(handle, 0x12, 0x06, 0xff)?;
                #[cfg(not(feature = "rtl_sdr_blog"))]
                self.write_reg_mask(handle, 0x12, 0x80, 0xe0)?;
            }
        }
        if (data[2] & 0x40) == 0 {
            info!("[R82xx] PLL not locked!");
            self.has_lock = false;
            return Ok(());
        }
        self.has_lock = true;

        // Set PLL auto-tune = 8kHz
        self.write_reg_mask(handle, 0x1a, 0x08, 0x08)?;
        Ok(())
    }

    fn sysfreq_sel(
        &mut self,
        handle: &Device,
        freq: u32,
        tuner_type: TunerType,
        delivery_system: DeliverySystem,
    ) -> Result<()> {
        let mixer_top;
        let lna_top;
        let cp_cur;
        let mut div_buf_cur;
        let lna_vth_l;
        let mixer_vth_l;
        let air_cable1_in;
        let cable2_in;
        let pre_dect;
        let lna_discharge;
        let filter_cur;

        match delivery_system {
            DeliverySystem::SysDvbt => {
                if (freq == 506000000) || (freq == 666000000) || (freq == 818000000) {
                    mixer_top = 0x14; /* mixer top:14 , top-1, low-discharge */
                    lna_top = 0xe5; /* detect bw 3, lna top:4, predet top:2 */
                    cp_cur = 0x28; /* 101, 0.2 */
                    div_buf_cur = 0x20; /* 10, 200u */
                } else {
                    mixer_top = 0x24; /* mixer top:13 , top-1, low-discharge */
                    lna_top = 0xe5; /* detect bw 3, lna top:4, predet top:2 */
                    cp_cur = 0x38; /* 111, auto */
                    div_buf_cur = 0x30; /* 11, 150u */
                }
                lna_vth_l = 0x53; /* lna vth 0.84	,  vtl 0.64 */
                mixer_vth_l = 0x75; /* mixer vth 1.04, vtl 0.84 */
                air_cable1_in = 0x00;
                cable2_in = 0x00;
                pre_dect = 0x40;
                lna_discharge = 14;
                filter_cur = 0x40; /* 10, low */
            }
            DeliverySystem::SysDvbt2 => {
                mixer_top = 0x24; /* mixer top:13 , top-1, low-discharge */
                lna_top = 0xe5; /* detect bw 3, lna top:4, predet top:2 */
                lna_vth_l = 0x53; /* lna vth 0.84	,  vtl 0.64 */
                mixer_vth_l = 0x75; /* mixer vth 1.04, vtl 0.84 */
                air_cable1_in = 0x00;
                cable2_in = 0x00;
                pre_dect = 0x40;
                lna_discharge = 14;
                cp_cur = 0x38; /* 111, auto */
                div_buf_cur = 0x30; /* 11, 150u */
                filter_cur = 0x40; /* 10, low */
            }
            DeliverySystem::SysIsdbt => {
                mixer_top = 0x24; /* mixer top:13 , top-1, low-discharge */
                lna_top = 0xe5; /* detect bw 3, lna top:4, predet top:2 */
                lna_vth_l = 0x75; /* lna vth 1.04	,  vtl 0.84 */
                mixer_vth_l = 0x75; /* mixer vth 1.04, vtl 0.84 */
                air_cable1_in = 0x00;
                cable2_in = 0x00;
                pre_dect = 0x40;
                lna_discharge = 14;
                cp_cur = 0x38; /* 111, auto */
                div_buf_cur = 0x30; /* 11, 150u */
                filter_cur = 0x40; /* 10, low */
            }
            DeliverySystem::SysUndefined => {
                // DVB-T 8M
                mixer_top = 0x24; /* mixer top:13 , top-1, low-discharge */
                lna_top = 0xe5; /* detect bw 3, lna top:4, predet top:2 */
                lna_vth_l = 0x53; /* lna vth 0.84	,  vtl 0.64 */
                mixer_vth_l = 0x75; /* mixer vth 1.04, vtl 0.84 */
                air_cable1_in = 0x00;
                cable2_in = 0x00;
                pre_dect = 0x40;
                lna_discharge = 14;
                cp_cur = 0x38; /* 111, auto */
                div_buf_cur = 0x30; /* 11, 150u */
                filter_cur = 0x40; /* 10, low */
            }
        }
        if self.use_predetect {
            self.write_reg_mask(handle, 0x06, pre_dect, 0x40)?;
        }
        self.write_reg_mask(handle, 0x1d, lna_top, 0xc7)?;
        self.write_reg_mask(handle, 0x1c, mixer_top, 0xf8)?;
        self.write_regs(handle, 0x0d, &[lna_vth_l])?;
        self.write_regs(handle, 0x0e, &[mixer_vth_l])?;

        // Air-IN only for Astrometa
        self.write_reg_mask(handle, 0x05, air_cable1_in, 0x60)?;
        self.write_reg_mask(handle, 0x06, cable2_in, 0x08)?;
        self.write_reg_mask(handle, 0x11, cp_cur, 0x38)?;

        // RTLSDRBLOG. Improve L-band performance by setting PLL drop out to 2.0v
        #[cfg(feature = "rtl_sdr_blog")]
        {
            div_buf_cur = 0xa0;
        }

        self.write_reg_mask(handle, 0x17, div_buf_cur, 0x30)?;
        self.write_reg_mask(handle, 0x0a, filter_cur, 0x60)?;

        // Set LNA
        if !matches!(tuner_type, TunerType::TunerAnalogTv) {
            // LNA TOP: lowest
            self.write_reg_mask(handle, 0x1d, 0, 0x38)?;
            // 0: normal mode
            self.write_reg_mask(handle, 0x1c, 0, 0x04)?;
            // 0: PRE_DECT off
            self.write_reg_mask(handle, 0x06, 0, 0x40)?;
            // agc clk 250hz
            self.write_reg_mask(handle, 0x1a, 0x30, 0x30)?;

            // write LNA TOP = 3
            self.write_reg_mask(handle, 0x1d, 0x18, 0x38)?;

            /*
             * write discharge mode
             * FIXME: IMHO, the mask here is wrong, but it matches
             * what's there at the original driver
             */
            self.write_reg_mask(handle, 0x1c, mixer_top, 0x04)?;
            // LNA discharge current
            self.write_reg_mask(handle, 0x1e, lna_discharge, 0x1f)?;
            // agc clk 60hz
            self.write_reg_mask(handle, 0x1a, 0x20, 0x30)?;
        } else {
            // PRE_DECT off
            self.write_reg_mask(handle, 0x06, 0, 0x40)?;
            // write LNA TOP
            self.write_reg_mask(handle, 0x1d, lna_top, 0x38)?;

            /*
             * write discharge mode
             * FIXME: IMHO, the mask here is wrong, but it matches
             * what's there at the original driver
             */
            self.write_reg_mask(handle, 0x1c, mixer_top, 0x04)?;
            // LNA discharge current
            self.write_reg_mask(handle, 0x1e, lna_discharge, 0x1f)?;
            // agc clk 1Khz, external det1 cap 1u
            self.write_reg_mask(handle, 0x1a, 0x00, 0x30)?;
        }
        self.write_reg_mask(handle, 0x10, lna_discharge, 0x04)?;
        Ok(())
    }

    fn set_tv_standard(&mut self, handle: &Device, _bw: u32, tuner_type: TunerType) -> Result<()> {
        /* BW < 6 MHz */
        let if_khz = 3570;
        let filt_cal_lo = 56000; /* 52000->56000 */
        let filt_gain = 0x10; /* +3db, 6mhz on */
        let img_r = 0x00; /* image negative */
        let filt_q = 0x10; /* r10[4]:low q(1'b1) */
        let hp_cor = 0x6b; /* 1.7m disable, +2cap, 1.0mhz */
        let ext_enable = 0x60; /* r30[6]=1 ext enable; r30[5]:1 ext at lna max-1 */
        let loop_through = 0x01; /* r5[7], lt off */
        let lt_att = 0x00; /* r31[7], lt att enable */
        let flt_ext_widest = 0x00; /* r15[7]: flt_ext_wide off */
        let polyfil_cur = 0x60; /* r25[6:5]:min */

        // Initialize register cache
        self.regs.copy_from_slice(&REG_INIT[0..NUM_CACHE_REGS]);

        // Init Flag & Xtal_check Result (inits VGA gain, needed?)
        self.write_reg_mask(handle, 0x0c, 0x00, 0x0f)?;

        // Version
        self.write_reg_mask(handle, 0x13, VER_NUM, 0x3f)?;

        // for LT Gain test
        if !matches!(tuner_type, TunerType::TunerAnalogTv) {
            self.write_reg_mask(handle, 0x1d, 0x00, 0x38)?;
        }
        self.int_freq = if_khz * 1000;

        /* Check if standard changed. If so, filter calibration is needed */
        /* Since we call this function only once in rtlsdr, force calibration */
        let need_calibration = true;
        if need_calibration {
            for _ in 0..2 {
                // Set filt_cap
                self.write_reg_mask(handle, 0x0b, hp_cor, 0x60)?;
                // set cali clk = on
                self.write_reg_mask(handle, 0x0f, 0x04, 0x04)?;
                // X'tal cap 0pF for PLL
                self.write_reg_mask(handle, 0x10, 0x00, 0x03)?;

                self.set_pll(handle, filt_cal_lo * 1000)?;

                // Start trigger
                self.write_reg_mask(handle, 0x0b, 0x10, 0x10)?;
                // Stop trigger
                self.write_reg_mask(handle, 0x0b, 0x00, 0x04)?;

                // Check if calibration worked
                let mut data: [u8; 5] = [0; 5];
                self.read_reg(handle, 0x00, &mut data, 5)?;
                self.fil_cal_code = data[4] & 0x0f;
                if self.fil_cal_code & self.fil_cal_code != 0x0f {
                    break;
                }
                // Narrowest
                if self.fil_cal_code == 0x0f {
                    self.fil_cal_code = 0;
                }
            }
        }
        self.write_reg_mask(handle, 0x0a, filt_q | self.fil_cal_code, 0x1f)?;

        // Set BW, Filter_gain, and HP corner
        self.write_reg_mask(handle, 0x0b, hp_cor, 0xef)?;

        // Set Img_R
        self.write_reg_mask(handle, 0x07, img_r, 0x80)?;

        // Set filt_3dB, V6MHz
        self.write_reg_mask(handle, 0x06, filt_gain, 0x30)?;

        // Channel filter extension
        self.write_reg_mask(handle, 0x1e, ext_enable, 0x60)?;

        // Loop through
        self.write_reg_mask(handle, 0x05, loop_through, 0x80)?;

        // Loop through attenuation
        self.write_reg_mask(handle, 0x1f, lt_att, 0x80)?;

        // Filter extension widest
        self.write_reg_mask(handle, 0x0f, flt_ext_widest, 0x80)?;

        // RF poly filter current
        self.write_reg_mask(handle, 0x19, polyfil_cur, 0x60)?;

        // Original driver stores delivery sys and tuner type, but never uses it again
        Ok(())
    }

    fn _xtal_check(&mut self, handle: &Device) -> Result<u8> {
        let mut data: [u8; 3] = [0; 3];

        // Initialize register cache
        for i in RW_REG_START..NUM_REGS {
            self.regs[i] = REG_INIT[i];
        }

        // cap 30pF & Drive Low
        self.write_reg_mask(handle, 0x10, 0x0b, 0x0b)?;
        // set pll autotune = 128kHz
        self.write_reg_mask(handle, 0x1a, 0x00, 0x0c)?;
        // set manual initial reg = 111111;
        self.write_reg_mask(handle, 0x13, 0x7f, 0x7f)?;
        // set auto
        self.write_reg_mask(handle, 0x13, 0x00, 0x40)?;

        // Try several xtal capacitor alternatives
        for cap_val in XTAL_CAPACITOR_VALUES.iter() {
            self.write_reg_mask(handle, 0x10, *cap_val, 0x1b)?;
            self.read_reg(handle, 0x00, &mut data, 3)?;
            if data[2] & 0x40 == 0 {
                continue;
            }

            let val = data[2] & 0x3f;
            if (self.xtal == 16_000_000 && (val > 29 || val < 23)) || val != 0x3f {
                return Ok(*cap_val);
            }
        }
        Err(RtlsdrErr(format!(
            "Unable to find good xtal capacitor value!"
        )))
    }

    /// Write register with bit-masked data
    fn write_reg_mask(&mut self, handle: &Device, reg: usize, val: u8, bit_mask: u8) -> Result<()> {
        let rc = self.read_cache_reg(reg);
        // Compute the desired register value: (rc & !mask) gets the unmasked bits and leaves the masked as 0,
        // and (val & mask) gets just the masked bits we want to set. Or together to get the desired register.
        let applied: u8 = (rc & !bit_mask) | (val & bit_mask);
        Ok(self.write_regs(handle, reg, &[applied])?)
    }

    /// Read register data from local cache
    /// # Panics
    ///     * reg < RW_REG_START
    ///     * reg > NUM_REGS
    fn read_cache_reg(&self, reg: usize) -> u8 {
        assert!(reg >= RW_REG_START); // is assert the best thing to use here?
        let index = reg - RW_REG_START;
        assert!(index < NUM_CACHE_REGS); // is assert the best thing to use here?
        self.regs[index]
    }

    /// Write data to device registers (r82xx_write)
    fn write_regs(&mut self, handle: &Device, reg: usize, val: &[u8]) -> Result<()> {
        // Store write in local cache
        self.reg_cache_store(reg, val);

        // Use I2C to write to device in chunks of MAX_I2C_MSG_LEN
        let mut len = val.len();
        let mut val_index = 0;
        let mut reg_index = reg;
        loop {
            // First byte in message is the register addr, then the data
            let size = if len > MAX_I2C_MSG_LEN - 1 {
                MAX_I2C_MSG_LEN
            } else {
                len
            };
            let mut buf: Vec<u8> = vec![0; size + 1];
            buf[0] = reg_index as u8;
            buf[1..].copy_from_slice(&val[val_index..val_index + size]);
            handle.i2c_write(R820T_I2C_ADDR, &buf)?;
            val_index += size;
            reg_index += size;
            len -= size;
            if len <= 0 {
                break;
            }
        }
        Ok(())
    }

    // (r82xx_read)
    fn read_reg(&self, handle: &Device, reg: usize, buf: &mut [u8], len: u8) -> Result<()> {
        assert!(buf.len() >= len as usize);
        handle.i2c_write(R820T_I2C_ADDR, &[reg as u8])?;
        handle.i2c_read(R820T_I2C_ADDR, buf, len)?;
        // Need to reverse each byte...for some reason?
        for i in 0..buf.len() {
            buf[i] = bit_reverse(buf[i]);
        }
        Ok(())
    }

    /// Cache register values locally.
    /// Will panic if reg < RW_REG_START or (reg + len) > NUM_CACHE_REGS + 1
    fn reg_cache_store(&mut self, mut reg: usize, val: &[u8]) {
        assert!(reg >= RW_REG_START);
        reg = reg - RW_REG_START;
        assert!(reg + val.len() <= NUM_CACHE_REGS);
        self.regs[reg..reg + val.len()].copy_from_slice(val);
    }
}

fn bit_reverse(byte: u8) -> u8 {
    const LUT: [u8; 16] = [
        0x0, 0x8, 0x4, 0xc, 0x2, 0xa, 0x6, 0xe, 0x1, 0x9, 0x5, 0xd, 0x3, 0xb, 0x7, 0xf,
    ];
    (LUT[(byte & 0xf) as usize] << 4) | LUT[(byte >> 4) as usize]
}
