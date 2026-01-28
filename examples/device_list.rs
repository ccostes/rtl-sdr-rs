// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Example demonstrating device enumeration and opening by serial number
//!
//! This example shows how to:
//! - List all available RTL-SDR devices
//! - Get device information (serial numbers, product names, etc.)
//! - Open a device by its serial number
//! - Open the first available device

use rtl_sdr_rs::RtlSdr;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("RTL-SDR Device Enumeration Example");
    println!("===================================\n");

    // Get the number of devices
    let count = RtlSdr::get_device_count()?;
    println!("Found {} RTL-SDR device(s)\n", count);

    if count == 0 {
        println!("No RTL-SDR devices found. Please connect a device and try again.");
        return Ok(());
    }

    // List all devices with their information
    println!("Device List:");
    println!("-----------");
    let devices = RtlSdr::list_devices()?;
    for device in &devices {
        println!("Device #{}:", device.index);
        println!("  Manufacturer: {}", device.manufacturer);
        println!("  Product:      {}", device.product);
        println!("  Serial:       {}", device.serial);
        println!("  VID:PID:      {:04x}:{:04x}", device.vendor_id, device.product_id);
        println!();
    }

    // Example 1: Open the first available device
    println!("Example 1: Opening first available device...");
    match RtlSdr::open_first_available() {
        Ok(mut sdr) => {
            println!("✓ Successfully opened first device");
            println!("  Center Frequency: {} Hz", sdr.get_center_freq());
            println!("  Sample Rate:      {} Hz", sdr.get_sample_rate());
            sdr.close()?;
        }
        Err(e) => {
            println!("✗ Failed to open device: {}", e);
        }
    }
    println!();

    // Example 2: Open device by index
    println!("Example 2: Opening device by index 0...");
    match RtlSdr::open_with_index(0) {
        Ok(mut sdr) => {
            println!("✓ Successfully opened device at index 0");
            println!("  Center Frequency: {} Hz", sdr.get_center_freq());
            sdr.close()?;
        }
        Err(e) => {
            println!("✗ Failed to open device: {}", e);
        }
    }
    println!();

    // Example 3: Open device by serial number
    if !devices.is_empty() {
        let serial = &devices[0].serial;
        println!("Example 3: Opening device by serial number '{}'...", serial);
        match RtlSdr::open_with_serial(serial) {
            Ok(mut sdr) => {
                println!("✓ Successfully opened device with serial '{}'", serial);
                println!("  Center Frequency: {} Hz", sdr.get_center_freq());
                println!("  Sample Rate:      {} Hz", sdr.get_sample_rate());
                sdr.close()?;
            }
            Err(e) => {
                println!("✗ Failed to open device: {}", e);
            }
        }
        println!();
    }

    // Example 4: Get device information without opening
    println!("Example 4: Getting device info for index 0...");
    match RtlSdr::get_device_info(0) {
        Ok(info) => {
            println!("✓ Device information retrieved:");
            println!("  Serial: {}", info.serial);
            println!("  Product: {}", info.product);
        }
        Err(e) => {
            println!("✗ Failed to get device info: {}", e);
        }
    }
    println!();

    // Example 5: Get just the serial number
    println!("Example 5: Getting serial number for index 0...");
    match RtlSdr::get_device_serial(0) {
        Ok(serial) => {
            println!("✓ Serial number: {}", serial);
        }
        Err(e) => {
            println!("✗ Failed to get serial: {}", e);
        }
    }

    Ok(())
}

