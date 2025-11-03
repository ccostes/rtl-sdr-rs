use ctrlc;
use rtl_sdr_rs::{error::Result, DeviceDescriptors, DeviceId, RtlSdr, DEFAULT_BUF_LENGTH};
use std::env;
use std::sync::atomic::{AtomicBool, Ordering};

const SAMPLE_RATE: u32 = 2_048_000;

fn main() -> Result<()> {
    let mut dev_id_str = "0".to_string();
    let args: Vec<String> = env::args().collect();
    if let Some(index) = args.iter().position(|arg| arg == "-d") {
        if let Some(val) = args.get(index + 1) {
            dev_id_str = val.clone();
        }
    }

    let device_descriptors = DeviceDescriptors::new()?.iter().collect::<Vec<_>>();
    if device_descriptors.is_empty() {
        eprintln!("No supported devices found.");
        return Ok(());
    }

    println!("Found {} device(s):", device_descriptors.len());
    for dev in &device_descriptors {
        println!(
            "  {}:  {}, {}, SN: {}",
            dev.index, dev.manufacturer, dev.product, dev.serial
        );
    }
    println!();

    let device_desc_to_open = if let Ok(index) = dev_id_str.parse::<usize>() {
        device_descriptors.iter().find(|d| d.index == index)
    } else {
        device_descriptors.iter().find(|d| d.serial == dev_id_str)
    };

    let Some(descriptor) = device_desc_to_open else {
        eprintln!("No matching device found for '{}'.", dev_id_str);
        return Ok(());
    };

    println!(
        "Using device {}: {}, {}, SN: {}",
        descriptor.index, descriptor.manufacturer, descriptor.product, descriptor.serial
    );

    let mut sdr = RtlSdr::open(DeviceId::Index(descriptor.index))?;

    println!("Found {} tuner", sdr.get_tuner_id()?);

    let gains = sdr.get_tuner_gains()?;
    print!("Supported gain values ({}):", gains.len());
    for g in gains {
        print!(" {:.1}", g as f32 / 10.0);
    }
    println!();

    sdr.set_sample_rate(SAMPLE_RATE)?;
    println!("Sampling at {} S/s.", sdr.get_sample_rate());

    sdr.set_testmode(true)?;
    sdr.reset_buffer()?;
    println!("Reading samples in sync mode...");

    static SHUTDOWN: AtomicBool = AtomicBool::new(false);
    ctrlc::set_handler(|| SHUTDOWN.store(true, Ordering::Relaxed))
        .expect("Error setting Ctrl-C handler");

    let mut buf = vec![0u8; DEFAULT_BUF_LENGTH];
    while !SHUTDOWN.load(Ordering::Relaxed) {
        match sdr.read_sync(&mut buf) {
            Ok(n) if n < DEFAULT_BUF_LENGTH => {
                eprintln!("Short read ({:#?}), samples lost, exiting!", n);
                break;
            }
            Err(e) => {
                eprintln!("Read error: {:#?}", e);
                break;
            }
            _ => {}
        }
    }

    println!("\nClosing device...");
    sdr.close()?;
    Ok(())
}
