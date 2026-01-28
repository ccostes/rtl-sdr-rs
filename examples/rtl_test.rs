/// Command-line options:
///
/// - `--device <index>` or `-d <index>`: Selects the RTL-SDR device to use by its index (as listed at startup).
///   - Example: `--device 0`
///   - Mutually exclusive with `--find/-f`.
///
/// - `--find <filters>` or `-f <filters>`: Selects the RTL-SDR device to use by matching key-value filters.
///   - Filters are comma-separated pairs in the form `key=value`.
///   - Supported keys: `manufacturer`, `product`, `serial`.
///   - Example: `--find manufacturer=Realtek,product=RTL2838UHIDIR,serial=00000001`
///   - Mutually exclusive with `--device/-d`.
///
/// If neither option is specified, the program will print an error and exit.
///
/// The program lists all detected devices at startup, showing their index, manufacturer, product, and serial number.
use ctrlc;
use rtl_sdr_rs::{error::Result, DeviceDescriptors, DeviceId, RtlSdr, DEFAULT_BUF_LENGTH};
use std::collections::HashMap;
use std::env;
use std::sync::atomic::{AtomicBool, Ordering};

const SAMPLE_RATE: u32 = 2_048_000;

#[derive(Debug)]
enum DeviceMode {
    Index,
    Find,
}

#[derive(Debug, PartialEq, Eq, Hash)]
enum FilterKey {
    Manufacturer,
    Product,
    Serial,
}

fn parse_key_value_pairs(input: &str) -> HashMap<FilterKey, String> {
    input
        .split(',')
        .filter_map(|pair| {
            let mut parts = pair.splitn(2, '=');
            if let (Some(key), Some(value)) = (parts.next(), parts.next()) {
                let key_enum = match key {
                    "manufacturer" => Some(FilterKey::Manufacturer),
                    "product" => Some(FilterKey::Product),
                    "serial" => Some(FilterKey::Serial),
                    _ => panic!(
                        "Unknown filter key: {}, must be one of manufacturer, product, serial",
                        key
                    ),
                };
                key_enum.map(|k| (k, value.to_string()))
            } else {
                None
            }
        })
        .collect()
}

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();

    let mut device_mode: Option<DeviceMode> = None;
    let mut device_value = String::new();

    let mut args_iter = args.iter();

    while let Some(arg) = args_iter.next() {
        match arg.as_str() {
            "--device" | "-d" => {
                if device_mode.is_some() {
                    eprintln!("Error: --device/-d and --find/-f are mutually exclusive.");
                    return Ok(());
                }
                device_mode = Some(DeviceMode::Index);
                if let Some(value) = args_iter.next() {
                    device_value = value.clone();
                }
            }
            "--find" | "-f" => {
                if device_mode.is_some() {
                    eprintln!("Error: --device/-d and --find/-f are mutually exclusive.");
                    return Ok(());
                }
                device_mode = Some(DeviceMode::Find);
                if let Some(value) = args_iter.next() {
                    device_value = value.clone();
                }
            }
            _ => {}
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

    let device_desc_to_open = match device_mode {
        Some(DeviceMode::Index) => {
            if let Ok(index) = device_value.parse::<usize>() {
                eprintln!("Opening device by index: {}", index);
                device_descriptors.iter().find(|d| d.index == index)
            } else {
                eprintln!("Invalid index value: '{}'.", device_value);
                return Ok(());
            }
        }
        Some(DeviceMode::Find) => {
            let filters = parse_key_value_pairs(&device_value);
            eprintln!("Searching for device with filters: {:?}", filters);
            device_descriptors.iter().find(|d| {
                filters.iter().all(|(key, value)| match key {
                    FilterKey::Manufacturer => d.manufacturer == *value,
                    FilterKey::Product => d.product == *value,
                    FilterKey::Serial => d.serial == *value,
                })
            })
        }
        _ => {
            eprintln!("No device selection mode specified. Use --device/-d or --find/-f.");
            return Ok(());
        }
    };

    let Some(descriptor) = device_desc_to_open else {
        eprintln!("No matching device found for '{}'.", device_value);
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
