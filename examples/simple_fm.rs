/// Simple FM demodulator example with hard-coded params. Single-
/// threaded using synchronous reads, so intended for instructional
/// purposes rather than performance. Should work like the original
/// rtl_fm outputting raw audio data that can be piped to sox play.
/// 
/// Can also read data from a file instead of real rtl-sdr device by
/// setting READ_FROM_FILE to true.
/// 
/// cargo run --example simple_fm | play -r 32k -t raw -e s -b 16 -c 1 -V1 -

use log::{info};
use std::io::{Write};
use ctrlc;
use std::sync::atomic::{AtomicBool, Ordering};
use rtlsdr_rs::{RtlSdr, DEFAULT_BUF_LENGTH};
use core::alloc::Layout;
use std::alloc::alloc_zeroed;
use num_complex::Complex;
use std::f64::consts::PI;

// Switch whether to use a real device or read raw data from file
const READ_FROM_FILE: bool = false;             
const INPUT_FILE_PATH: &str = "capture.bin";

const DEFAULT_FREQUENCY: u32 = 92_500_000;  // Frequency in Hz, 120.9MHz
const SAMPLE_RATE: u32 = 170_000;           // Demodulation sample rate, 12kHz 
const RATE_RESAMPLE: u32 = 32_000;          // Demodulation sample rate, 12kHz 
const MAXIMUM_OVERSAMPLE: usize = 16;
const MAXIMUM_BUF_LENGTH: usize = (MAXIMUM_OVERSAMPLE * DEFAULT_BUF_LENGTH);

fn main() {
    stderrlog::new().verbosity(log::Level::Info).init().unwrap();

    // Create shutdown flag and set it when ctrl-c signal caught
    static shutdown: AtomicBool = AtomicBool::new(false);
    ctrlc::set_handler(|| {shutdown.swap(true, Ordering::Relaxed);} );

    let mut demod = Demod::new();

    if !READ_FROM_FILE {
        // Open device
        let mut sdr = RtlSdr::open();
        // Get settings
        let (freq, rate ) = demod.optimal_settings(DEFAULT_FREQUENCY, SAMPLE_RATE);
        // Config receiver
        config_sdr(&mut sdr, freq, rate);

        info!("Tuned to {} Hz.\n", sdr.get_center_freq());
        info!("Oversampling input by: {}x", demod.downsample);
        info!("Buffer size: {}ms", 1000.0 * 0.5 * DEFAULT_BUF_LENGTH as f32 / rate as f32);
        info!("Sampling at {} S/s", sdr.get_sample_rate());
        info!("Output at {} Hz", demod.rate_in);
        info!("Output scale: {}", demod.output_scale);

        info!("Reading samples in sync mode...");
        let mut buf: Box<[u8; DEFAULT_BUF_LENGTH]> = alloc_buf();
        loop {
            if shutdown.load(Ordering::Relaxed) {
                break;
            }
            let n = sdr.read_sync(&mut *buf);
            if n.is_err() {
                info!("Read error: {:#?}", n);
            } else if n.unwrap() < DEFAULT_BUF_LENGTH {
                info!("Short read ({:#?}), samples lost, exiting!", n);
                break;
            }
            let len = n.unwrap();
            demod.receive(&mut *buf, len);
            demod.output();
        }
        info!("Close");
        sdr.close();
    } else {
        // Read raw data from file instead of real device
        use std::io::prelude::*;
        use std::fs::File;
        let mut f = File::open(INPUT_FILE_PATH).expect("failed to open file");
        let mut buf = [0_u8; DEFAULT_BUF_LENGTH];
        demod.optimal_settings(DEFAULT_FREQUENCY, SAMPLE_RATE);
        loop {
            if shutdown.load(Ordering::Relaxed) {
                break;
            }
            let n = f.read(&mut buf[..]).expect("failed to read");
            demod.receive(&mut buf, n);
            demod.output();
        }
    }
}

fn config_sdr(sdr: &mut RtlSdr, freq: u32, rate: u32) {
    sdr.set_tuner_gain(rtlsdr_rs::TunerGain::AUTO);
    // Disable bias-tee
    sdr.set_bias_tee(false);
    // Reset the endpoint before we try to read from it (mandatory)
    sdr.reset_buffer();
    // Set the frequency
    sdr.set_center_freq(freq);
    // Set sample rate
    sdr.set_sample_rate(rate);
}

struct Demod {
    buf16: Box<[u16; MAXIMUM_BUF_LENGTH]>,
    lowpassed: Box<[i16; MAXIMUM_BUF_LENGTH]>,
    lp_len: usize,
    result: Box<[i16; MAXIMUM_BUF_LENGTH]>,
    result_len: usize,
    rate_in: u32,
    rate_out: u32,
    rate_resample: u32,
    downsample: u32,
    pre_r: i16,
    pre_j: i16,
    now_r: i16,
    now_j: i16,
    prev_index: usize,
    output_scale: u32,
    now_lpr: i32,
    prev_lpr_index: i32,
}

impl Demod {
    fn new() -> Self {
        Demod { 
            buf16: alloc_buf(),
            lowpassed: alloc_buf(), 
            lp_len: 0, 
            result: alloc_buf(), 
            result_len: 0, 
            rate_in: SAMPLE_RATE, 
            rate_out: SAMPLE_RATE, 
            rate_resample: RATE_RESAMPLE,
            downsample: 0, 
            pre_r: 0,
            pre_j: 0,
            now_r: 0, 
            now_j: 0, 
            prev_index: 0, 
            output_scale: 0,
            now_lpr: 0,
            prev_lpr_index: 0,
        }
    }

    fn optimal_settings(&mut self, freq: u32, rate: u32) -> (u32, u32) {
        self.downsample = (1_000_000 / self.rate_in) + 1;
        info!("downsample: {}", self.downsample);
        let capture_rate = self.downsample * self.rate_in;
        info!("rate_in: {} capture_rate: {}", self.rate_in, capture_rate);
        // Use offset-tuning
        let capture_freq = freq + capture_rate / 4;
        info!("capture_freq: {}", capture_freq);
        self.output_scale = (1 << 15) / (128 * self.downsample);
        if self.output_scale < 1 {
            self.output_scale = 1;
        }
        (capture_freq, capture_rate)
    }

    fn low_pass_real(&mut self) {
        // Simple square-window FIR
        let slow = self.rate_resample;
        let fast = self.rate_out;
        let mut i = 0;
        let mut i2 = 0;
        while i < self.result_len {
            self.now_lpr += self.result[i] as i32;
            i += 1;
            self.prev_lpr_index += slow as i32;
            if self.prev_lpr_index < fast as i32 {
                continue;
            }
            self.result[i2] = (self.now_lpr / ((fast / slow) as i32)) as i16;
            self.prev_lpr_index -= fast as i32;
            self.now_lpr = 0;
            i2 += 1;
        }
        self.result_len = i2;
    }

    fn low_pass(&mut self) -> usize{
        let mut res = 0;
        for orig in (0..self.lp_len).step_by(2) {
            self.now_r += self.lowpassed[orig];
            self.now_j += self.lowpassed[orig + 1];
            
            self.prev_index += 1;
            if self.prev_index < self.downsample as usize {
                continue;
            }

            self.lowpassed[res] = self.now_r;
            self.lowpassed[res + 1] = self.now_j;
            res += 2;
            self.prev_index = 0;
            self.now_r = 0;
            self.now_j = 0;
        }
        res
    }

    fn polar_discriminant(a: Complex<i32>, b: Complex<i32>) -> i32 {
        let c = a * b.conj();
        let angle = f64::atan2(c.im as f64, c.re as f64);
        (angle / PI * (1 << 14) as f64) as i32
    }

    fn polar_discriminant_fast(a: Complex<i32>, b: Complex<i32>) -> i32 {
        let c = a * b.conj();
        Demod::fast_atan2(c.im, c.re)
    }

    fn fast_atan2(y: i32, x: i32) -> i32 {
        // Pre-scaled for i16
        // pi = 1 << 14
        let pi4 = (1 << 12);
        let pi34 = 3 * (1 << 12);
        if x == 0 && y == 0 {
            return 0;
        }
        let mut yabs = y;
        if yabs < 0 {
            yabs = -yabs;
        }
        let angle;
        if x >= 0 {
            angle = pi4 - (pi4 as i64 * (x - yabs) as i64) as i32 / (x + yabs);
        } else {
            angle = pi34 - (pi4 as i64 * (x + yabs) as i64) as i32 / (yabs - x);
        }
        if y < 0 {
            return -angle;
        }
        return angle;
    }
    
    fn rotate_90(mut buf: &mut[u8], len: usize)
    /* 90 rotation is 1+0j, 0+1j, -1+0j, 0-1j
       or [0, 1, -3, 2, -4, -5, 7, -6] */
    {
        let mut tmp: u8;
        for i in (0..len).step_by(8) {
            /* uint8_t negation = 255 - x */
            tmp = 255 - buf[i+3];
            buf[i+3] = buf[i+2];
            buf[i+2] = tmp;
    
            buf[i+4] = 255 - buf[i+4];
            buf[i+5] = 255 - buf[i+5];
    
            tmp = 255 - buf[i+6];
            buf[i+6] = buf[i+7];
            buf[i+7] = tmp;
        }
    }

    fn fm_demod(&mut self) {
        let mut pcm = Demod::polar_discriminant(
            Complex::new(self.lowpassed[0] as i32, self.lowpassed[1] as i32), 
            Complex::new(self.pre_r as i32, self.pre_j as i32));
        // info!("pre_r: {} pre_j: {} pcm: {}", self.pre_r, self.pre_j, pcm);
        self.result[0] = pcm as i16;
        for i in (2..self.lp_len - 1).step_by(2) {
            pcm = Demod::polar_discriminant_fast(
                Complex::new(self.lowpassed[i] as i32, self.lowpassed[i+1] as i32), 
                Complex::new(self.lowpassed[i-2] as i32, self.lowpassed[i-1] as i32));
            self.result[i/2] = pcm as i16;
        }
        self.pre_r = self.lowpassed[self.lp_len - 2];
        self.pre_j = self.lowpassed[self.lp_len - 1];
        self.result_len = self.lp_len / 2;
    }

    fn receive(&mut self, buf: &mut [u8], len: usize) {
        Demod::rotate_90(buf, len);
        for i in 0..len {
            let s = buf[i] as i16 - 127;
            self.buf16[i] = s as u16;
            self.lowpassed[i] = s;
        }
        self.lp_len = len;

        // low-pass filter to downsample to our desired sample rate
        self.lp_len = self.low_pass();

        // Demodulate FM signal
        self.fm_demod();

        // Resample
        self.low_pass_real();
    }

    fn output(&self) {
        // Write results to stdout
        use std::{mem, slice};
        let mut out = std::io::stdout();
        let slice_u8: &[u8] = unsafe {
            slice::from_raw_parts(
                self.result.as_ptr() as *const u8,
                self.result_len * mem::size_of::<i16>(),
            )
        };
        out.write_all(slice_u8);
        out.flush();
    }
}

fn alloc_buf<T>() -> Box<T> {
    let layout: Layout = Layout::new::<T>();
    // TODO move to using safe code once we can allocate an array directly on the heap.
    unsafe {
        let ptr = alloc_zeroed(layout) as *mut T;
        Box::from_raw(ptr)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_demod() {
        // End-to-end test of FM demod, based on data from rtl_fm
        // rtl_fm -f 92.5M -M fm -s 170k -A fast -r 32k -l 0
        const len:usize = 512;
        let mut buf: [u8; len] = [198, 94, 98, 135, 109, 119, 129, 133, 151, 138, 136, 133, 116, 123, 156, 122, 160, 151, 144, 133, 120, 122, 134, 136, 127, 125, 137, 120, 114, 125, 123, 137, 121, 134, 102, 146, 144, 149, 132, 143, 136, 205, 149, 152, 121, 110, 114, 144, 120, 130, 136, 135, 116, 141, 136, 130, 105, 128, 121, 127, 136, 136, 127, 122, 129, 145, 105, 125, 132, 117, 109, 127, 100, 129, 129, 121, 148, 124, 101, 113, 117, 118, 143, 132, 140, 155, 121, 115, 119, 143, 172, 122, 160, 163, 196, 136, 169, 90, 141, 125, 153, 122, 137, 117, 136, 117, 115, 135, 131, 118, 118, 136, 97, 122, 112, 99, 147, 131, 138, 134, 140, 106, 145, 113, 125, 138, 126, 139, 115, 120, 135, 129, 147, 131, 115, 139, 107, 122, 145, 122, 139, 115, 106, 99, 121, 110, 112, 105, 42, 84, 84, 169, 96, 134, 135, 123, 131, 140, 123, 144, 134, 101, 133, 109, 167, 102, 139, 154, 79, 109, 120, 111, 126, 146, 139, 109, 136, 121, 103, 140, 117, 105, 151, 104, 127, 106, 133, 121, 123, 140, 126, 133, 139, 120, 125, 141, 107, 143, 135, 149, 110, 124, 117, 200, 147, 148, 112, 127, 129, 136, 149, 104, 137, 117, 104, 135, 92, 121, 118, 129, 115, 130, 131, 134, 96, 137, 141, 96, 147, 118, 102, 129, 115, 134, 111, 124, 118, 117, 133, 140, 122, 136, 146, 138, 118, 149, 135, 116, 183, 99, 138, 139, 140, 130, 106, 129, 128, 178, 187, 137, 102, 93, 70, 119, 161, 127, 118, 113, 142, 132, 114, 122, 121, 128, 162, 155, 116, 165, 126, 104, 149, 111, 162, 156, 124, 143, 129, 83, 157, 89, 145, 122, 153, 124, 120, 120, 115, 120, 148, 145, 111, 131, 109, 129, 117, 122, 119, 111, 141, 112, 100, 82, 87, 86, 147, 143, 125, 111, 153, 97, 169, 129, 104, 140, 100, 144, 145, 115, 127, 113, 148, 163, 138, 136, 142, 107, 119, 103, 95, 146, 125, 137, 122, 111, 111, 126, 99, 136, 124, 140, 127, 128, 117, 103, 122, 107, 134, 113, 132, 111, 158, 147, 135, 155, 151, 213, 178, 194, 117, 114, 143, 125, 125, 121, 153, 171, 93, 126, 95, 111, 106, 130, 108, 128, 148, 124, 142, 128, 137, 153, 129, 130, 122, 122, 116, 148, 161, 108, 79, 120, 132, 174, 148, 152, 129, 137, 152, 118, 157, 143, 131, 150, 156, 107, 142, 104, 135, 142, 183, 139, 191, 102, 135, 136, 112, 139, 128, 130, 144, 140, 120, 172, 125, 129, 145, 131, 152, 129, 146, 110, 125, 120, 159, 130, 131, 118, 100, 124, 119, 104, 84, 129, 94, 137, 99, 136, 104, 132, 119, 124, 139, 130, 130, 164, 158, 138, 100, 109, 178, 134, 123, 111, 80, 109, 81, 165, 117, 115, 116, 120, 128, 110, 129, 145, 129, 118, 144, 125, 131, 126, 139, 108, 113, 101, 123, 112, 135, 155, 169, 99, 104, 120, 114, 147, 138, 145, 110, 112, 134, 131, 133, 141];
        let buf16 = vec![71, 65503, 65529, 65507, 19, 9, 6, 65535, 24, 11, 65531, 9, 12, 5, 65531, 65508, 33, 24, 65531, 17, 8, 6, 9, 65530, 0, 65534, 8, 10, 14, 3, 10, 5, 65530, 7, 65518, 65511, 65520, 65515, 16, 65532, 9, 78, 65512, 22, 7, 18, 17, 14, 65529, 3, 65529, 9, 12, 65523, 3, 65528, 65514, 1, 1, 65530, 65528, 65528, 65531, 1, 2, 18, 3, 65514, 65532, 11, 0, 19, 65509, 2, 7, 2, 65516, 4, 65522, 27, 65526, 65527, 65532, 16, 65524, 65509, 65524, 7, 65528, 16, 6, 45, 65504, 65501, 9, 65468, 42, 65499, 3, 14, 65511, 6, 65526, 65527, 9, 65526, 65529, 65524, 65533, 10, 9, 10, 65506, 65531, 29, 65521, 65517, 65533, 7, 65526, 13, 65515, 15, 18, 3, 65526, 12, 2, 65524, 65529, 65535, 8, 65517, 65533, 12, 13, 65516, 65531, 6, 18, 65525, 13, 65508, 22, 65530, 65519, 23, 65521, 86, 44, 42, 44, 65505, 7, 5, 8, 65533, 65524, 17, 5, 7, 65510, 19, 6, 65497, 26, 27, 65525, 65488, 65518, 17, 65529, 2, 65518, 65518, 65525, 9, 65530, 65524, 65512, 11, 23, 65513, 65513, 0, 65515, 7, 6, 5, 65524, 6, 2, 12, 65529, 65523, 65534, 21, 65521, 22, 65529, 65519, 65533, 65464, 65526, 65517, 65516, 0, 16, 2, 9, 24, 22, 65527, 11, 8, 24, 65501, 65530, 65535, 65527, 13, 65534, 7, 65533, 65505, 10, 32, 14, 65517, 10, 2, 26, 65524, 7, 4, 65520, 10, 11, 13, 65531, 65531, 9, 65526, 19, 10, 65515, 65525, 65529, 56, 65508, 65525, 11, 65524, 65534, 2, 22, 1, 51, 65527, 60, 26, 35, 65528, 58, 34, 0, 15, 65527, 65522, 65532, 65531, 14, 65530, 1, 65509, 35, 12, 65499, 65513, 2, 22, 65520, 65508, 35, 4, 65521, 65492, 65535, 30, 65498, 6, 18, 65511, 4, 65529, 8, 65524, 65529, 65519, 21, 17, 65533, 2, 19, 65526, 65531, 17, 65528, 65523, 16, 65491, 28, 65496, 65495, 65521, 20, 3, 17, 65506, 65511, 42, 2, 65524, 65513, 28, 65520, 65524, 65519, 0, 65522, 65501, 21, 65526, 65528, 65516, 65522, 65528, 65512, 65518, 65504, 3, 65527, 65520, 6, 65520, 65535, 65528, 65508, 4, 65524, 1, 1, 65526, 65512, 21, 65531, 65530, 15, 65520, 65532, 31, 20, 65509, 8, 65513, 65451, 67, 65486, 65526, 65523, 3, 16, 3, 7, 44, 65511, 65502, 65535, 17, 65504, 22, 65534, 1, 20, 21, 65533, 0, 15, 65527, 65511, 3, 65535, 65531, 65531, 65516, 65525, 65503, 20, 65529, 49, 5, 47, 65512, 21, 65535, 65527, 65527, 65512, 30, 16, 65514, 4, 65508, 21, 65513, 65522, 8, 15, 65525, 56, 65473, 26, 9, 65529, 65521, 12, 65534, 1, 65520, 65524, 45, 8, 65534, 2, 65533, 18, 65512, 65535, 65519, 65518, 65534, 65529, 65534, 32, 65533, 10, 65533, 28, 65528, 65513, 65535, 65493, 34, 65527, 9, 29, 65513, 5, 4, 65528, 65525, 65534, 37, 65534, 31, 11, 19, 65509, 65486, 65530, 65520, 5, 65489, 65518, 65499, 65490, 11, 13, 65529, 12, 1, 65519, 65519, 2, 65535, 10, 65534, 65520, 4, 65535, 20, 12, 15, 27, 65521, 5, 8, 28, 29, 42, 24, 8, 20, 14, 11, 18, 16, 65519, 65530, 65533, 14, 65531];
        let lowpass  = vec![108, -34, 52, 18, 8, -2, 9, 107, -20, -14, -12, 19, -68, 42, -49, -62, 12, -48, -7, -13, 30, -10, -60, 58, 119, 71, 28, -12, -50, -84, 6, -25, -47, -44, 6, 62, -15, 4, -2, 33, 29, -17, 0, 224, -3, 37, -57, -32, -25, 6, -32, 47, -52, -50, -49, -48, -63, -88, -6, -29, 41, -104, 53, -33, -10, -30, -69, 104, -46, 98, -42, 28, -50, 26, 28, -8, 57, -23, -146, -40, 5, -10, 81, 124];
        // let demod  = vec![0, 3489, -3236, 9337, 11916, -8564, 2688, 7340, 4624, -3906, 9406, 13730, -9938, -4746, -9153, 4043, -5222, -12548, 7028, -6147, -11481, 11220, 615, 10771, -3940, -3900, 9381, 76, 1228, 2517, 3241, 3490, -6608, -11786, -1057, 3088, 805, -14996, -783, -12842, 9551, 11213];
        let result  = vec![2588, 4030, -1212, -3430, 2585, 2110, -6110, ];
        let mut demod = Demod::new();

        demod.optimal_settings(DEFAULT_FREQUENCY, SAMPLE_RATE);
        demod.receive(&mut buf, len);
        assert_eq!(buf16, demod.buf16[0..len]);
        assert_eq!(lowpass, demod.lowpassed[0..demod.lp_len]);
        assert_eq!(result, demod.result[0..demod.result_len]);
    }
}