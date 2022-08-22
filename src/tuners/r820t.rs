use super::{Tuner, TunerInfo, TunerGainMode};
use crate::usb::RtlSdrDeviceHandle;

const R820T_I2C_ADDR: u16 = 0x34;
// const R828D_I2C_ADDR: u8 = 0x74; for now only support the T

const R82XX_IF_FREQ: u32 = 3570000;
const NUM_REGS: usize = 30;
const REG_SHADOW_START: usize = 5;
const MAX_I2C_MSG_LEN: usize = 8;

struct FreqRange {
    freq: u32,          // Start freq, in MHz
    open_d: u8,         // low
    rf_mux_ploy: u8,    // R26[7:6]=0 (LPF)  R26[1:0]=2 (low)
    tf_c: u8,           // R27[7:0]  band2,band0
    xtal_cap20p: u8,    // R16[1:0]  20pF (10)
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

enum Xtal_Cap_Value {
	XTAL_LOW_CAP_30P,
	XTAL_LOW_CAP_20P,
	XTAL_LOW_CAP_10P,
	XTAL_LOW_CAP_0P,
	XTAL_HIGH_CAP_0P,    
}

pub struct R820T {
    pub tuner: TunerInfo,
    regs: Vec<u8>,
    pub freq: u32,
    int_freq: u32,
    xtal_cap_sel: Xtal_Cap_Value,
    xtal: u32,
    has_lock: bool,
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
    pub fn new(handle: &mut RtlSdrDeviceHandle) -> R820T {
        let tuner = R820T { 
            tuner: TUNER_INFO, 
            regs: Vec::with_capacity(NUM_REGS),
            freq: 0,
            int_freq: 0,
            xtal_cap_sel: Xtal_Cap_Value::XTAL_LOW_CAP_30P,
            xtal: 0,
            has_lock: false,
        };
        tuner.init(handle);
        tuner
    }
}
    
impl Tuner for R820T {
    fn init(&self, handle: &RtlSdrDeviceHandle) {
        // disable Zero-IF mode
        handle.demod_write_reg(1, 0xb1, 0x1a, 1);

        // only enable In-phase ADC input
        handle.demod_write_reg(0, 0x08, 0x4d, 1);

        // the R82XX use 3.57 MHz IF for the DVB-T 6 MHz mode, and
        // 4.57 MHz for the 8 MHz mode
        handle.set_if_freq(R82XX_IF_FREQ);

        // enable spectrum inversion
        handle.demod_write_reg(1, 0x15, 0x01, 1);
    }

    fn set_gain_mode(&mut self, handle: &RtlSdrDeviceHandle, mode: TunerGainMode) {
        self.set_gain(handle, mode, 0);
    }

    fn set_freq(&mut self, handle: &RtlSdrDeviceHandle, freq: u32) {
        let lo_freq = freq + self.int_freq;
        self.set_mux(handle, lo_freq);
        self.set_pll(handle, lo_freq);
    }
}

impl R820T {

    fn set_gain(&mut self, handle: &RtlSdrDeviceHandle, mode: TunerGainMode, gain: i32) {
        match mode {
            TunerGainMode::AUTO => {
                // LNA
                self.write_reg_mask(handle, 0x05, 0, 0x10);
                // Mixer
                self.write_reg_mask(handle, 0x07, 0x10, 0x10);
                // Set fixed VGA gain for now (26.5 dB)
                self.write_reg_mask(handle, 0x0c, 0x0b, 0x9f);
            },
            TunerGainMode::MANUAL(gain) => {
                // TODO: set manual gain
            }
        }
    }

    // Tuning logic

    fn set_mux(&mut self, handle: &RtlSdrDeviceHandle, freq: u32) {
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
        self.write_reg_mask(handle, 0x17, range.open_d, 0x08);

        // RF_MUX, Polymux
        self.write_reg_mask(handle, 0x1a, range.rf_mux_ploy, 0xc3);

        // TF Band
        self.write_regs(handle, 0x1b, &[range.tf_c]);

        // XTAL CAP & Drive
        let val = match self.xtal_cap_sel {
            Xtal_Cap_Value::XTAL_LOW_CAP_30P | Xtal_Cap_Value::XTAL_LOW_CAP_20P => {
                range.xtal_cap20p | 0x08
            },
            Xtal_Cap_Value::XTAL_LOW_CAP_10P => {
                range.xtal_cap10p | 0x08
            },
            Xtal_Cap_Value::XTAL_HIGH_CAP_0P => {
                range.xtal_cap0p | 0x00
            },
            Xtal_Cap_Value::XTAL_LOW_CAP_0P | _ => {
                range.xtal_cap0p | 0x08
            }
        };
        self.write_reg_mask(handle, 0x10, val, 0x0b);
        self.write_reg_mask(handle, 0x08, 0x00, 0x3f);
        self.write_reg_mask(handle, 0x09, 0x00, 0x3f);
    }

    fn set_pll(&mut self, handle: &RtlSdrDeviceHandle, freq: u32) {
        // Frequency in kHz
        let freq_khz = (freq + 500) / 1000;
        let pll_ref = self.xtal;
        let pll_ref_khz = (self.xtal + 500) / 1000;

        let refdiv2 = 0;
        self.write_reg_mask(handle, 0x10, refdiv2, 0x10);

        // Set PLL auto-tune = 128kHz
        self.write_reg_mask(handle, 0x1a, 0x00, 0x0c);

        // Set VCO current = 100 (RTL-SDR Blog Mod: MAX CURRENT)
        self.write_reg_mask(handle, 0x12, 0x06, 0xff);

        // Test turning tracking filter off
        // self.write_reg_mask(handle, 0x1a, 0x40, 0xc0);

        // Calculate divider
        let vco_min: u32 = 1770000;
        let vco_max: u32 = vco_min * 2;
        let mut mix_div: u8 = 2;
        let mut div_buf: u8 = 0;
        let mut div_num: u8 = 0;
        while mix_div <= 64 {
            if ((freq_khz * mix_div as u32) >= vco_min) && ((freq_khz * mix_div as u32) < vco_max) {
                div_buf = mix_div;
                while div_buf > 2 {
                    div_buf = div_buf >> 1;
                    div_num += 1;
                }
                break;
            }
            mix_div = mix_div << 1;
        }
        
        let mut data: [u8;5] = [0;5];
        self.read_reg(handle, 0x00, &mut data, 5);
        // TODO: if chip is R828D set vco_power_ref = 1
        let vco_power_ref = 2;
        let vco_fine_tune = (data[4] & 0x30) >> 4;
        if vco_fine_tune > vco_power_ref {
            div_num = div_num - 1;
        } else if vco_fine_tune < vco_power_ref {
            div_num = div_num + 1;
        }
        self.write_reg_mask(handle, 0x10, div_num << 5, 0xe0);

        let vco_freq = freq as u64 * mix_div as u64;
        let nint = (vco_freq / (2 * pll_ref as u64)) as u8;
        let mut vco_fra = ((vco_freq - 2 * pll_ref as u64 * nint as u64) / 1000) as u32; // VCO contribution by SDM (kHz)
        if nint > ((128 / vco_power_ref) - 1) {
            println!("[R82xx] No valid PLL values for {} Hz!", freq);
            // TODO: Err here
        }
        let ni = (nint - 13) / 4;
        let si = nint - 4 * ni - 13;
        self.write_regs(handle, 0x14, &[ni + (si << 6)]);

        // pw_sdm
        if vco_fra == 0 {
            self.write_reg_mask(handle, 0x12, 0x08, 0x08);
        } else {
            self.write_reg_mask(handle, 0x12, 0x00, 0x08);
        }

        // SDM Calculator
        let mut sdm = 0;
        let n_sdm = 2;
        while vco_fra > 1 {
            if vco_fra > (2 * pll_ref_khz / n_sdm) {
                sdm = sdm + 32768 / (n_sdm / 2);
                vco_fra = vco_fra - 2 * pll_ref_khz / n_sdm;
                if n_sdm >= 0x8000 {
                    break;
                }
            }
            n_sdm << 1;
        }
        self.write_regs(handle, 0x16, &[(sdm >> 8) as u8]);
        self.write_regs(handle, 0x15, &[(sdm & 0xff) as u8]);
        for i in 0..2 {
            // Check if PLL has locked
            self.read_reg(handle, 0x00, &mut data, 3);
            if data[2] & 0x40 != 0 {
                break;
            }
            if i == 0 {
                // Didn't lock, increase VCO current
               self.write_reg_mask(handle, 0x12, 0x06, 0xff);
            }
        }
        if (data[2] & 0x40) == 0 {
            println!("[R82xx] PLL not locked!");
            self.has_lock = false;
            return ;
        }
        self.has_lock = true;

        // Set PLL auto-tune = 8kHz
        self.write_reg_mask(handle, 0x1a, 0x08, 0x08);
    }
    
    /// Write register with bit-masked data
    fn write_reg_mask(&mut self, handle: &RtlSdrDeviceHandle, reg: usize, val: u8, bit_mask: u8) {
        let rc = self.read_cache_reg(reg);
        // Compute the desired register value: (rc & !mask) gets the unmasked bits and leaves the masked as 0,
        // and (val & mask) gets just the masked bits we want to set. Or together to get the desired register.
        let applied: u8 = (rc & !bit_mask) | (val & bit_mask);
        self.write_regs(handle, reg, &[applied]);
    }

    /// Read register data from local cache
    fn read_cache_reg(&self, reg: usize) -> u8 {
        let index = reg - REG_SHADOW_START;
        assert!(index >= 0 && index < NUM_REGS); // is assert the best thing to use here?
        self.regs[index]
    }
    
    /// Write data to device regiers
    fn write_regs(&mut self, handle: &RtlSdrDeviceHandle, reg: usize, val: &[u8]) {
        // Store write in local cache
        self.shadow_store(reg, val);
        
        // Use I2C to write to device in chunks of MAX_I2C_MSG_LEN
        let mut len = val.len();
        let mut val_index = 0;
        let mut reg_index = reg;
        loop {
            // First byte in message is the register addr, then the data
            let size = if len > MAX_I2C_MSG_LEN - 1 { MAX_I2C_MSG_LEN } else { len };
            let mut buf: Vec<u8> = Vec::with_capacity(size + 1);
            buf[0] = reg_index as u8;
            buf[1..].copy_from_slice(&val[val_index..val_index+size]);
            handle.i2c_write(R820T_I2C_ADDR, &buf);
            val_index += size;
            reg_index += size;
            len -= size;
        }
    }

    fn read_reg(&self, handle: &RtlSdrDeviceHandle, reg: usize, buf: &mut[u8], len: u8) {
        assert!(buf.len() >= len as usize);
        handle.i2c_write(R820T_I2C_ADDR, &[reg as u8]);
        handle.i2c_read(R820T_I2C_ADDR, buf, len);
        // Need to reverse each byte...for some reason?
        for i in 0..buf.len() {
            buf[i] = bit_reverse(buf[i]);
        }
    }

    /// Cache register values locally. Will panic if reg < REG_SHADOW_START or (reg + len) > NUM_REG 
    fn shadow_store(&mut self, reg: usize, val: &[u8]) {
        assert!(reg < REG_SHADOW_START);
        assert!(reg + val.len() <= NUM_REGS);
        let index = reg - REG_SHADOW_START;
        self.regs[reg..reg + val.len()].copy_from_slice(val);
    }

}

fn bit_reverse(byte: u8) -> u8 {
    const lut: [u8;16] = [ 0x0, 0x8, 0x4, 0xc, 0x2, 0xa, 0x6, 0xe,
        0x1, 0x9, 0x5, 0xd, 0x3, 0xb, 0x7, 0xf ];
    (lut[(byte & 0xf) as usize] << 4) | lut[(byte >> 4) as usize]
}