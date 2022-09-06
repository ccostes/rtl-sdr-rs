use rtlsdr_rs::RtlSdr;
use rusb::{Context, Device, DeviceHandle, Result, UsbContext, Error};
use rtlsdr_rs::usb::RtlSdrDeviceHandle;

enum TestMode {
    NO_BENCHMARK,
    TUNER_BENCHMARK,
    PPM_BENCHMARK,
}

const FREQUENCY: u32 = 120_900_000;
const SAMPLE_RATE: u32 = 2_048_000;
const GAIN: rtlsdr_rs::TunerGain = rtlsdr_rs::TunerGain::AUTO;

fn main() -> Result<()> {
    // Open device
    let mut sdr = RtlSdr::open();
    // println!("{:#?}", sdr);

    let gains = sdr.get_tuner_gains();
    println!("Supported gain values ({}): {:?}", gains.len(), gains.iter().map(|g| {*g as f32 / 10.0}).collect::<Vec<_>>());

    // Set sample rate
    sdr.set_sample_rate(SAMPLE_RATE);
    println!("Sampling at {} S/s", sdr.get_sample_rate());

    // Enable test mode
    println!("Enable test mode");
    sdr.set_testmode(true);

    // Reset the endpoint before we try to read from it (mandatory)
    println!("Reset buffer");
    sdr.reset_buffer();

    println!("Close");
    sdr.close();

    // let (freq, rate) = optimal_settings(FREQUENCY, SAMPLE_RATE);
    // // set up primary channel
    // sdr.set_center_freq(FREQUENCY);
    // println!("Tuned to {} Hz", sdr.get_center_freq());

    Ok(())
}