use rtlsdr_rs::RtlSdr;
use rusb::{Context, Device, DeviceHandle, Result, UsbContext, Error};
mod usb;
use usb::RtlSdrDeviceHandle;

const FREQUENCY: u32 = 120_900_000;
const SAMPLE_RATE: u32 = 12_000;
const GAIN: rtlsdr_rs::TunerGainMode = rtlsdr_rs::TunerGainMode::AUTO;

fn main() -> Result<()> {
    // Open device
    let mut sdr = RtlSdr::open();

    // Set the tuner gain
    sdr.set_tuner_gain_mode(GAIN);
    // Disable bias-tee
    sdr.set_bias_tee(false);
    // Reset the endpoint before we try to read from it (mandatory)
    sdr.reset_buffer();

    let (freq, rate) = optimal_settings(FREQUENCY, SAMPLE_RATE);
    // set up primary channel
    sdr.set_center_freq(FREQUENCY);
    println!("Tuned to {} Hz", sdr.get_center_freq());
    // Set sample rate
    sdr.set_sample_rate(SAMPLE_RATE);
    println!("Sampling at {} S/s", sdr.get_sample_rate());


    Ok(())
}

fn sdr_init(sdr: &RtlSdr) {
}

fn optimal_settings(freq: u32, rate: u32) -> (u32, u32) {
    (0,0)
}