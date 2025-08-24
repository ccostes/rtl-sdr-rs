//! FM radio example with hard-coded params that outputs raw audio
//! data to stdout just like the original rtl_fm.
//!
//! Can also read raw data from a file instead of a real rtl-sdr device by
//! setting READ_FROM_FILE to true, which can be a good way to verify that
//! audio output is working.
//!
//! Example command to run the program and output audio with `play` (must be installed):
//! cargo run --example simple_fm | play -r 32k -t raw -e s -b 16 -c 1 -V1 -

use core::alloc::Layout;
use ctrlc;
use log::info;
use num_complex::Complex;
use rtl_sdr_rs::{error::Result, RtlSdr, DEFAULT_BUF_LENGTH, Args};
use std::alloc::alloc_zeroed;
use std::f64::consts::PI;
use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::{Instant, Duration};


// Radio and demodulation config
const FREQUENCY: u32 = 94_900_000; // Frequency in Hz, 91.1MHz WREK Atlanta
const SAMPLE_RATE: u32 = 170_000; // Demodulation sample rate, 170kHz
const RATE_RESAMPLE: u32 = 32_000; // Output sample rate, 32kHz

// Switch to read raw data from file instead of real device, and what file to read from.
// Setting this to true can be a quick way to verify that the program and audio output is working.
const READ_FROM_FILE: bool = false;
const INPUT_FILE_PATH: &str = "capture.bin";
// RTL Device Index
const RTL_INDEX: usize = 0;

fn main() {
    // Printing to stdout will break audio output, so use this to log to stderr instead
    stderrlog::new().verbosity(log::Level::Info).init().unwrap();

    // Shutdown flag that is set true when ctrl-c signal caught
    static SHUTDOWN: AtomicBool = AtomicBool::new(false);
    ctrlc::set_handler(|| {
        SHUTDOWN.swap(true, Ordering::Relaxed);
    })
    .unwrap();

    // Get radio and demodulation settings for given frequency and sample rate
    let (radio_config, demod_config) = optimal_settings(FREQUENCY, SAMPLE_RATE);

    // Check if configured to use real device or read from file
    if !READ_FROM_FILE {
        // Real device! Will use two threads, one to handle the SDR and one for demodulation and output

        // Channel to pass receive data from receiver thread to processor thread
        let (tx, rx) = mpsc::channel();

        // Spawn thread to receive data from Radio
        let receive_thread = thread::spawn(|| receive(&SHUTDOWN, radio_config, tx));
        // Spawn thread to process data and output to stdout
        let process_thread = thread::spawn(|| process(&SHUTDOWN, demod_config, rx));

        // Wait for threads to finish
        process_thread.join().unwrap();
        receive_thread.join().unwrap();
    } else {
        // Read raw data from file instead of real device
        use std::fs::File;
        use std::io::prelude::*;
        let mut f = File::open(INPUT_FILE_PATH).expect("failed to open file");
        let mut buf = [0_u8; DEFAULT_BUF_LENGTH];
        let mut demod = Demod::new(demod_config);
        loop {
            // Check if shutdown signal received
            if SHUTDOWN.load(Ordering::Relaxed) {
                break;
            }
            // Read chunk of file  data into buf
            let n = f.read(&mut buf[..]).expect("failed to read");
            // Demodulate data from file
            let result = demod.demodulate(buf.to_vec());
            // Output resulting audio data to stdout
            output(result);
        }
    }
}

/// Thread to open SDR device and send received data to the demod thread until
/// SHUTDOWN flag is set to true.
fn receive(shutdown: &AtomicBool, radio_config: RadioConfig, tx: Sender<Vec<u8>>) {
    // Open device
    let mut sdr = RtlSdr::open(Args::Index(RTL_INDEX)).expect("Failed to open device");
    // Config receiver
    config_sdr(
        &mut sdr,
        radio_config.capture_freq,
        radio_config.capture_rate,
    )
    .unwrap();

    info!("Tuned to {} Hz.\n", sdr.get_center_freq());
    info!(
        "Buffer size: {}ms",
        1000.0 * 0.5 * DEFAULT_BUF_LENGTH as f32 / radio_config.capture_rate as f32
    );
    info!("Sampling at {} S/s", sdr.get_sample_rate());

    info!("Reading samples in sync mode...");
    loop {
        // Check if SHUTDOWN flag is true and break out of the loop if so
        if shutdown.load(Ordering::Relaxed) {
            break;
        }
        // Allocate a buffer to store received data
        let mut buf: Box<[u8; DEFAULT_BUF_LENGTH]> = alloc_buf();
        // Receive data from SDR device
        let n = sdr.read_sync(&mut *buf);
        if n.is_err() {
            info!("Read error: {:#?}", n);
            break;
        }
        let len = n.unwrap();
        if len < DEFAULT_BUF_LENGTH {
            info!("Short read ({:#?}), samples lost, exiting!", len);
            break;
        }
        // Send received data through the channel to the processor thread
        tx.send(buf.to_vec());
    }
    // Shut down the device and exit
    info!("Close");
    sdr.close().unwrap();
}

/// Thread to process received data and output it to stdout
fn process(shutdown: &AtomicBool, demod_config: DemodConfig, rx: Receiver<Vec<u8>>) {
    // Create and configure demodulation struct
    let mut demod = Demod::new(demod_config);
    info!("Oversampling input by: {}x", demod.config.downsample);
    info!("Output at {} Hz", demod.config.rate_in);
    info!("Output scale: {}", demod.config.output_scale);

    // Variables to track the running average loop time
    let mut total_time: Duration = Duration::new(0, 0);
    let mut loop_count: u64 = 0;
    loop {
        if shutdown.load(Ordering::Relaxed) {
            break;
        }
        // Wait for data from the channel
        let buf = rx.recv().unwrap();
        // Demodulate data
        let start_time = Instant::now();
        let result = demod.demodulate(buf);
        let elapsed_time = start_time.elapsed();
        // Output audio data to stdout
        output(result);
        // Update total time and loop count for running average
        total_time += elapsed_time;
        loop_count += 1;
    }
    // Print the final average loop time when shutting down
    if loop_count > 0 {
        let final_avg_time = total_time.as_nanos() / loop_count as u128;
        info!("Average processing time: {:.2?}ms ({:?} loops)", final_avg_time as f32 / 1.0e6, loop_count);
    }
}

/// Radio configuration produced by `optimal_settings`
struct RadioConfig {
    capture_freq: u32,
    capture_rate: u32,
}

/// Demodulation configuration produced by `optimal_settings`
struct DemodConfig {
    rate_in: u32,       // Rate in Hz
    rate_out: u32,      // Rate in Hz
    rate_resample: u32, // Rate in Hz
    downsample: u32,
    output_scale: u32,
}

/// Determine the optimal radio and demodulation configurations for given
/// frequency and sample rate.
fn optimal_settings(freq: u32, rate: u32) -> (RadioConfig, DemodConfig) {
    let downsample = (1_000_000 / rate) + 1;
    info!("downsample: {}", downsample);
    let capture_rate = downsample * rate;
    info!("rate_in: {} capture_rate: {}", rate, capture_rate);
    // Use offset-tuning
    let capture_freq = freq + capture_rate / 4;
    info!("capture_freq: {}", capture_freq);
    let mut output_scale = (1 << 15) / (128 * downsample);
    if output_scale < 1 {
        output_scale = 1;
    }
    (
        RadioConfig {
            capture_freq: capture_freq,
            capture_rate: capture_rate,
        },
        DemodConfig {
            rate_in: SAMPLE_RATE,
            rate_out: SAMPLE_RATE,
            rate_resample: RATE_RESAMPLE,
            downsample: downsample,
            output_scale: output_scale,
        },
    )
}

/// Configure the SDR device for a given receive frequency and sample rate.
fn config_sdr(sdr: &mut RtlSdr, freq: u32, rate: u32) -> Result<()> {
    // Use auto-gain
    sdr.set_tuner_gain(rtl_sdr_rs::TunerGain::Auto)?;
    // Disable bias-tee
    sdr.set_bias_tee(false)?;
    // Reset the endpoint before we try to read from it (mandatory)
    sdr.reset_buffer()?;
    // Set the frequency
    sdr.set_center_freq(freq)?;
    // Set sample rate
    sdr.set_sample_rate(rate)?;
    Ok(())
}

/// State data for demodulation
struct Demod {
    config: DemodConfig,
    prev_index: usize,
    now_lpr: i32,
    prev_lpr_index: i32,
    lp_now: Complex<i32>,
    demod_pre: Complex<i32>,
}

/// Demodulation functions
impl Demod {
    fn new(config: DemodConfig) -> Self {
        Demod {
            config: config,
            prev_index: 0,
            now_lpr: 0,
            prev_lpr_index: 0,
            lp_now: Complex::new(0, 0),
            demod_pre: Complex::new(0, 0),
        }
    }

    /// Performs the entire demodulation process, given a vector of raw received bytes
    /// returns a vector of signed 16-bit audio data.
    fn demodulate(&mut self, mut buf: Vec<u8>) -> Vec<i16> {
        buf = Demod::rotate_90(buf);
        let buf_signed: Vec<i16> = buf.iter().map(|val| *val as i16 - 127).collect();
        let complex = buf_to_complex(buf_signed);
        // low-pass filter to downsample to our desired sample rate
        let lowpassed = self.low_pass_complex(complex);

        // Demodulate FM signal
        let demodulated = self.fm_demod(lowpassed);

        // Resample and return result
        let output = self.low_pass_real(demodulated);
        output
    }

    /// Performs a 90-degree rotation in the complex plane on a vector of bytes
    /// and returns the resulting vector.
    /// Data is assumed to be pairs of real and imaginary components.
    /// 90 rotation is 1+0j, 0+1j, -1+0j, 0-1j
    /// or rearranging elements according to [0, 1, -3, 2, -4, -5, 7, -6]
    fn rotate_90(mut buf: Vec<u8>) -> Vec<u8> {
        #[cfg(all(target_arch = "aarch64", not(feature = "disable-simd")))]
        {
            unsafe { Self::rotate_90_neon(buf) } // Use SIMD on ARM (NEON)
        }
        #[cfg(any(not(target_arch = "aarch64"), feature = "disable-simd"))]
        {
            let mut tmp: u8;
            for i in (0..buf.len()).step_by(8) {
                /* uint8_t negation = 255 - x */
                tmp = 255 - buf[i + 3];
                buf[i + 3] = buf[i + 2];
                buf[i + 2] = tmp;
    
                buf[i + 4] = 255 - buf[i + 4];
                buf[i + 5] = 255 - buf[i + 5];
    
                tmp = 255 - buf[i + 6];
                buf[i + 6] = buf[i + 7];
                buf[i + 7] = tmp;
            }
            buf
        }
    }
    #[cfg(target_arch = "aarch64")]
    unsafe fn rotate_90_neon(mut buf: Vec<u8>) -> Vec<u8> {
        use std::arch::aarch64::*;

        // Process 16 bytes (two sets of 8 bytes) per iteration
        for i in (0..buf.len()).step_by(16) {
            // Load two 8-byte chunks into NEON vectors
            let vec1 = vld1q_u8(&buf[i] as *const u8);      // First 8 bytes
            let vec2 = vld1q_u8(&buf[i + 8] as *const u8);  // Next 8 bytes

            // Apply the transformation for the first 8 bytes
            let mut result1 = vec1;
            result1 = vsetq_lane_u8(255 - vgetq_lane_u8(vec1, 3), result1, 2);
            result1 = vsetq_lane_u8(vgetq_lane_u8(vec1, 2), result1, 3);
            result1 = vsetq_lane_u8(255 - vgetq_lane_u8(vec1, 4), result1, 4);
            result1 = vsetq_lane_u8(255 - vgetq_lane_u8(vec1, 5), result1, 5);
            result1 = vsetq_lane_u8(255 - vgetq_lane_u8(vec1, 7), result1, 6);
            result1 = vsetq_lane_u8(vgetq_lane_u8(vec1, 6), result1, 7);

            // Apply the transformation for the next 8 bytes
            let mut result2 = vec2;
            result2 = vsetq_lane_u8(255 - vgetq_lane_u8(vec2, 3), result2, 2);
            result2 = vsetq_lane_u8(vgetq_lane_u8(vec2, 2), result2, 3);
            result2 = vsetq_lane_u8(255 - vgetq_lane_u8(vec2, 4), result2, 4);
            result2 = vsetq_lane_u8(255 - vgetq_lane_u8(vec2, 5), result2, 5);
            result2 = vsetq_lane_u8(255 - vgetq_lane_u8(vec2, 7), result2, 6);
            result2 = vsetq_lane_u8(vgetq_lane_u8(vec2, 6), result2, 7);

            // Store the results back to the buffer
            vst1q_u8(&mut buf[i] as *mut u8, result1);
            vst1q_u8(&mut buf[i + 8] as *mut u8, result2);
        }

        buf
    }

    /// Applies a low-pass filter on a vector of complex values
    fn low_pass_complex(&mut self, buf: Vec<Complex<i32>>) -> Vec<Complex<i32>> {
        let mut res = vec![];
        for orig in 0..buf.len() {
            self.lp_now += buf[orig];

            self.prev_index += 1;
            if self.prev_index < self.config.downsample as usize {
                continue;
            }

            res.push(self.lp_now);
            self.lp_now = Complex::new(0, 0);
            self.prev_index = 0;
        }
        res
    }

    /// Performs FM demodulation on a vector of complex input data
    fn fm_demod(&mut self, buf: Vec<Complex<i32>>) -> Vec<i16> {
        assert!(buf.len() > 1);
        let mut result = vec![];

        let mut pcm = Demod::polar_discriminant(buf[0], self.demod_pre);
        result.push(pcm as i16);
        for i in 1..buf.len() {
            pcm = Demod::polar_discriminant_fast(buf[i], buf[i - 1]);
            result.push(pcm as i16);
        }
        self.demod_pre = buf.last().copied().unwrap();
        result
    }

    /// Find the polar discriminant for a pair of complex values using real atan2 function
    fn polar_discriminant(a: Complex<i32>, b: Complex<i32>) -> i32 {
        let c = a * b.conj();
        let angle = f64::atan2(c.im as f64, c.re as f64);
        (angle / PI * (1 << 14) as f64) as i32
    }

    /// Find the polar discriminant for a pair of complex values using a fast atan2 approximation
    fn polar_discriminant_fast(a: Complex<i32>, b: Complex<i32>) -> i32 {
        let c = a * b.conj();
        Demod::fast_atan2(c.im, c.re)
    }

    /// Fast atan2 approximation
    fn fast_atan2(y: i32, x: i32) -> i32 {
        // Pre-scaled for i16
        // pi = 1 << 14
        let pi4 = 1 << 12;
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

    /// Applies a low-pass filter to a vector of real-valued data
    fn low_pass_real(&mut self, buf: Vec<i16>) -> Vec<i16> {
        let mut result = vec![];
        // Simple square-window FIR
        let slow = self.config.rate_resample;
        let fast = self.config.rate_out;
        let mut i = 0;
        while i < buf.len() {
            self.now_lpr += buf[i] as i32;
            i += 1;
            self.prev_lpr_index += slow as i32;
            if self.prev_lpr_index < fast as i32 {
                continue;
            }
            result.push((self.now_lpr / ((fast / slow) as i32)) as i16);
            self.prev_lpr_index -= fast as i32;
            self.now_lpr = 0;
        }
        result
    }
}

/// Write a vector of i16 values to stdout
fn output(buf: Vec<i16>) {
    use std::{mem, slice};
    let mut out = std::io::stdout();
    let slice_u8: &[u8] = unsafe {
        slice::from_raw_parts(buf.as_ptr() as *const u8, buf.len() * mem::size_of::<i16>())
    };
    out.write_all(slice_u8);
    out.flush();
}

/// Convert a vector of i16 complex components (real and imaginary) to a vector of i32 Complex values
fn buf_to_complex(buf: Vec<i16>) -> Vec<Complex<i32>> {
    buf
        // get overlapping windows of size 2
        .windows(2)
        // Step by 2 since we don't actually want overlapping windows
        .step_by(2)
        // Convert consecutive values to a single complex
        .map(|w| Complex::new(w[0] as i32, w[1] as i32))
        .collect()
}
/// Allocate a buffer on the heap
fn alloc_buf<T>() -> Box<T> {
    let layout: Layout = Layout::new::<T>();
    // TODO move to using safe code once we can allocate an array directly on the heap.
    unsafe {
        let ptr = alloc_zeroed(layout) as *mut T;
        Box::from_raw(ptr)
    }
}

// Tests for the major demodulation functions, using input/output data extracted from the original rtl_fm program
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_lowpass() {
        // Based on data from rtl_fm
        // rtl_fm -f 92.5M -M fm -s 170k -A fast -r 32k -l 0
        let lowpass = vec![
            108, -34, 52, 18, 8, -2, 9, 107, -20, -14, -12, 19, -68, 42, -49, -62, 12, -48, -7,
            -13, 30, -10, -60, 58, 119, 71, 28, -12, -50, -84, 6, -25, -47, -44, 6, 62, -15, 4, -2,
            33, 29, -17, 0, 224, -3, 37, -57, -32, -25, 6, -32, 47, -52, -50, -49, -48, -63, -88,
            -6, -29, 41, -104, 53, -33, -10, -30, -69, 104, -46, 98, -42, 28, -50, 26, 28, -8, 57,
            -23, -146, -40, 5, -10, 81, 124,
        ];
        let lp_complex = buf_to_complex(lowpass);

        let (_, demod_config) = optimal_settings(FREQUENCY, SAMPLE_RATE);
        let mut demod = Demod::new(demod_config);

        let buf_signed = vec![
            71, -33, -7, -29, 19, 9, 6, -1, 24, 11, -5, 9, 12, 5, -5, -28, 33, 24, -5, 17, 8, 6, 9,
            -6, 0, -2, 8, 10, 14, 3, 10, 5, -6, 7, -18, -25, -16, -21, 16, -4, 9, 78, -24, 22, 7,
            18, 17, 14, -7, 3, -7, 9, 12, -13, 3, -8, -22, 1, 1, -6, -8, -8, -5, 1, 2, 18, 3, -22,
            -4, 11, 0, 19, -27, 2, 7, 2, -20, 4, -14, 27, -10, -9, -4, 16, -12, -27, -12, 7, -8,
            16, 6, 45, -32, -35, 9, -68, 42, -37, 3, 14, -25, 6, -10, -9, 9, -10, -7, -12, -3, 10,
            9, 10, -30, -5, 29, -15, -19, -3, 7, -10, 13, -21, 15, 18, 3, -10, 12, 2, -12, -7, -1,
            8, -19, -3, 12, 13, -20, -5, 6, 18, -11, 13, -28, 22, -6, -17, 23, -15, 86, 44, 42, 44,
            -31, 7, 5, 8, -3, -12, 17, 5, 7, -26, 19, 6, -39, 26, 27, -11, -48, -18, 17, -7, 2,
            -18, -18, -11, 9, -6, -12, -24, 11, 23, -23, -23, 0, -21, 7, 6, 5, -12, 6, 2, 12, -7,
            -13, -2, 21, -15, 22, -7, -17, -3, -72, -10, -19, -20, 0, 16, 2, 9, 24, 22, -9, 11, 8,
            24, -35, -6, -1, -9, 13, -2, 7, -3, -31, 10, 32, 14, -19, 10, 2, 26, -12, 7, 4, -16,
            10, 11, 13, -5, -5, 9, -10, 19, 10, -21, -11, -7, 56, -28, -11, 11, -12, -2, 2, 22, 1,
            51, -9, 60, 26, 35, -8, 58, 34, 0, 15, -9, -14, -4, -5, 14, -6, 1, -27, 35, 12, -37,
            -23, 2, 22, -16, -28, 35, 4, -15, -44, -1, 30, -38, 6, 18, -25, 4, -7, 8, -12, -7, -17,
            21, 17, -3, 2, 19, -10, -5, 17, -8, -13, 16, -45, 28, -40, -41, -15, 20, 3, 17, -30,
            -25, 42, 2, -12, -23, 28, -16, -12, -17, 0, -14, -35, 21, -10, -8, -20, -14, -8, -24,
            -18, -32, 3, -9, -16, 6, -16, -1, -8, -28, 4, -12, 1, 1, -10, -24, 21, -5, -6, 15, -16,
            -4, 31, 20, -27, 8, -23, -85, 67, -50, -10, -13, 3, 16, 3, 7, 44, -25, -34, -1, 17,
            -32, 22, -2, 1, 20, 21, -3, 0, 15, -9, -25, 3, -1, -5, -5, -20, -11, -33, 20, -7, 49,
            5, 47, -24, 21, -1, -9, -9, -24, 30, 16, -22, 4, -28, 21, -23, -14, 8, 15, -11, 56,
            -63, 26, 9, -7, -15, 12, -2, 1, -16, -12, 45, 8, -2, 2, -3, 18, -24, -1, -17, -18, -2,
            -7, -2, 32, -3, 10, -3, 28, -8, -23, -1, -43, 34, -9, 9, 29, -23, 5, 4, -8, -11, -2,
            37, -2, 31, 11, 19, -27, -50, -6, -16, 5, -47, -18, -37, -46, 11, 13, -7, 12, 1, -17,
            -17, 2, -1, 10, -2, -16, 4, -1, 20, 12, 15, 27, -15, 5, 8, 28, 29, 42, 24, 8, 20, 14,
            11, 18, 16, -17, -6, -3, 14, -5,
        ];
        let complex = buf_to_complex(buf_signed);
        let lowpassed = demod.low_pass_complex(complex);
        assert_eq!(lp_complex, lowpassed);
    }

    #[test]
    fn test_demod() {
        // Based on data from rtl_fm
        // rtl_fm -f 92.5M -M fm -s 170k -A fast -r 32k -l 0
        let lowpass = vec![
            108, -34, 52, 18, 8, -2, 9, 107, -20, -14, -12, 19, -68, 42, -49, -62, 12, -48, -7,
            -13, 30, -10, -60, 58, 119, 71, 28, -12, -50, -84, 6, -25, -47, -44, 6, 62, -15, 4, -2,
            33, 29, -17, 0, 224, -3, 37, -57, -32, -25, 6, -32, 47, -52, -50, -49, -48, -63, -88,
            -6, -29, 41, -104, 53, -33, -10, -30, -69, 104, -46, 98, -42, 28, -50, 26, 28, -8, 57,
            -23, -146, -40, 5, -10, 81, 124,
        ];
        let lp_complex = buf_to_complex(lowpass);
        let demod_expected = vec![
            0, 3489, -3236, 9337, 11916, -8564, 2688, 7340, 4624, -3906, 9406, 13730, -9938, -4746,
            -9153, 4043, -5222, -12548, 7028, -6147, -11481, 11220, 615, 10771, -3940, -3900, 9381,
            76, 1228, 2517, 3241, 3490, -6608, -11786, -1057, 3088, 805, -14996, -783, -12842,
            9551, 11213,
        ];
        let result = vec![2588, 4030, -1212, -3430, 2585, 2110, -6110];

        let (_, demod_config) = optimal_settings(FREQUENCY, SAMPLE_RATE);
        let mut demod = Demod::new(demod_config);

        let demodulated = demod.fm_demod(lp_complex);
        assert_eq!(demod_expected, demodulated);
    }

    #[test]
    fn test_lowpass_real() {
        let demodulated = vec![
            0, 3489, -3236, 9337, 11916, -8564, 2688, 7340, 4624, -3906, 9406, 13730, -9938, -4746,
            -9153, 4043, -5222, -12548, 7028, -6147, -11481, 11220, 615, 10771, -3940, -3900, 9381,
            76, 1228, 2517, 3241, 3490, -6608, -11786, -1057, 3088, 805, -14996, -783, -12842,
            9551, 11213,
        ];
        let result = vec![2588, 4030, -1212, -3430, 2585, 2110, -6110];

        let (_, demod_config) = optimal_settings(FREQUENCY, SAMPLE_RATE);
        let mut demod = Demod::new(demod_config);

        let output = demod.low_pass_real(demodulated);
        assert_eq!(result, output);
    }
}
