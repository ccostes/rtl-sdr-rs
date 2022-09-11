/// Simple AM demodulator example with hard-coded params 
/// demonstrating synchronous processing. Should work like the
/// original rtl_fm outputting raw audio data that can be piped
/// to sox play.

use std::io::{self, Write};
use ctrlc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use rtlsdr_rs::{RtlSdr, DEFAULT_BUF_LENGTH};
use core::alloc::Layout;
use std::alloc::alloc_zeroed;

const DEFAULT_FREQUENCY: u32 = 120_000_000;  // Frequency in Hz, 120.9MHz
const SAMPLE_RATE: u32 = 12000;              // Demodulation sample rate, 12kHz 
const MAXIMUM_OVERSAMPLE: usize = 16;
const MAXIMUM_BUF_LENGTH: usize = (MAXIMUM_OVERSAMPLE * DEFAULT_BUF_LENGTH);

fn alloc_buf<T>() -> Box<T> {
    let layout: Layout = Layout::new::<T>();
    unsafe {
        let ptr = alloc_zeroed(layout) as *mut T;
        Box::from_raw(ptr)
    }
}

struct Demod {
    buf16: Box<[u16; MAXIMUM_BUF_LENGTH]>,
    lowpassed: Box<[i16; MAXIMUM_BUF_LENGTH]>,
    lp_len: usize,
    result: Box<[i16; MAXIMUM_BUF_LENGTH]>,
    result_len: usize,
    rate_in: u32,
    rate_out: u32,
    downsample: u32,
    now_r: i16,
    now_j: i16,
    prev_index: usize,
    output_scale: u32,
    squelch_level: i32,
    conseq_squelch: i32,
    squelch_hits: i32,
    terminate_on_squelch: bool,
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
            downsample: 0, 
            now_r: 0, 
            now_j: 0, 
            prev_index: 0, 
            output_scale: 0, 
            squelch_level: 0, 
            conseq_squelch: 10, 
            squelch_hits: 11, 
            terminate_on_squelch: false
        }
    }

    fn optimal_settings(&mut self, freq: u32, rate: u32) -> (u32, u32) {
        self.downsample = (1_000_000 / self.rate_in) + 1;
        let capture_rate = self.downsample * self.rate_in;
        let capture_freq = freq + capture_rate / 4;
        self.output_scale = (1 << 15) / (128 * self.downsample);
        if self.output_scale < 1 {
            self.output_scale = 1;
        }
        (capture_freq, capture_rate)
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

        // power squelch
        if self.squelch_level > 0 {
            let sr = Demod::rms(&*self.lowpassed, self.lp_len, 1);
            if sr < self.squelch_level {
                self.squelch_hits += 1;
                for i in 0..self.lp_len {
                    self.lowpassed[i] = 0;
                }
            } else {
                self.squelch_hits = 0;
            }
        }

        if self.squelch_level > 0 && self.squelch_hits > self.conseq_squelch {
            self.squelch_hits = self.conseq_squelch + 1; // hair trigger
            return ;
        }

        // Demodulate AM signal
        self.am_demod();

        // Write results to stdout
        use std::{mem, slice};
        let mut out = std::io::stdout();
        let slice_u8: &[u8] = unsafe {
            slice::from_raw_parts(
                self.result.as_ptr() as *const u8,
                self.result.len() * mem::size_of::<u16>(),
            )
        };
        out.write_all(slice_u8);
        out.flush();
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

    fn am_demod(&mut self) {
        for i in (0..self.lp_len).step_by(2) {
            let pcm = (self.lowpassed[i] as i64 * self.lowpassed[i+1] as i64) + 
                (self.lowpassed[i+1] as i64);
            self.result[i/2] = (f32::sqrt(pcm as f32) * self.output_scale as f32) as i16;
        }
        self.result_len = self.lp_len / 2;
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

    fn rms(samples: &[i16], len: usize, step: usize) -> i32 {
        let (mut p, mut t, mut s): (f64, f64, f64) = (0.0,0.0,0.0);
        for i in (0..len).step_by(step) {
            s = samples[i] as f64;
            t += s;
            p += s * s;
        }
        // Correct for DC offset in squares
        let dc = (t * step as f64) / len as f64;
        let err = t * 2.0 * dc - dc * dc * len as f64;
        f64::sqrt((p - err) / len as f64) as i32
    }

}

fn main() {
    // Create shutdown flag and set it when ctrl-c signal caught
    static shutdown: AtomicBool = AtomicBool::new(false);
    ctrlc::set_handler(|| {shutdown.swap(true, Ordering::Relaxed);} );

    let mut demod = Demod::new();

    // Open device
    let mut sdr = RtlSdr::open();

    // TODO: manual gain argument
    sdr.set_tuner_gain(rtlsdr_rs::TunerGain::AUTO);

    // Disable bias-tee
    sdr.set_bias_tee(false);
    
    // Reset the endpoint before we try to read from it (mandatory)
    println!("Reset buffer");
    sdr.reset_buffer();

    // Set up primary channel
    let (rate, freq) = demod.optimal_settings(DEFAULT_FREQUENCY, SAMPLE_RATE);

    // Set the frequency 
    sdr.set_center_freq(freq);
    println!("Tuned to {} Hz.\n", sdr.get_center_freq());
    println!("Oversampling input by: {}x", demod.downsample);
    println!("Buffer size: {}ms", 1000.0 * 0.5 * DEFAULT_BUF_LENGTH as f32 / SAMPLE_RATE as f32);

    // Set sample rate
    sdr.set_sample_rate(rate);
    println!("Sampling at {} S/s", sdr.get_sample_rate());
    println!("Output at {} Hz", demod.rate_in);
    println!("Output scale: {}", demod.output_scale);
    println!("Reading samples in sync mode...");
    // TODO move to using safe code once we can allocate an array directly on the heap.
    let mut buf: Box<[u8; DEFAULT_BUF_LENGTH]> = alloc_buf();
    loop {
        if shutdown.load(Ordering::Relaxed) {
            break;
        }
        let n = sdr.read_sync(&mut *buf);
        if n.is_err() {
            println!("Read error: {:#?}", n);
        } else if n.unwrap() < DEFAULT_BUF_LENGTH {
            println!("Short read ({:#?}), samples lost, exiting!", n);
            break;
        }
        let len = n.unwrap();
        demod.receive(&mut *buf, len);
    }
    println!("Close");
    sdr.close();
}