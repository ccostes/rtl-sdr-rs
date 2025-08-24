//! Example demonstrating the different ways to open an RTL-SDR device
//! using the new Args enum API.

use rtl_sdr_rs::{Args, RtlSdr};

fn main() {
    println!("RTL-SDR Args Demo");
    println!("================");
    
    // Method 1: Using Args::Index directly  
    println!("1. Opening device using Args::Index(0):");
    match RtlSdr::open(Args::Index(0)) {
        Ok(_sdr) => println!("   ✓ Successfully opened device with index 0"),
        Err(e) => println!("   ✗ Failed to open device: {}", e),
    }
    
    // Method 2: Using convenience function for index
    println!("2. Opening device using convenience function open_with_index(0):");
    match RtlSdr::open_with_index(0) {
        Ok(_sdr) => println!("   ✓ Successfully opened device with index 0"),
        Err(e) => println!("   ✗ Failed to open device: {}", e),
    }
    
    // Method 3: Using file descriptor (will fail unless you have a real fd)
    println!("3. Opening device using Args::Fd(42) - this will likely fail:");
    match RtlSdr::open(Args::Fd(42)) {
        Ok(_sdr) => println!("   ✓ Successfully opened device with fd 42"),
        Err(e) => println!("   ✗ Failed to open device: {}", e),
    }
    
    // Method 4: Using convenience function for fd  
    println!("4. Opening device using convenience function open_with_fd(42):");
    match RtlSdr::open_with_fd(42) {
        Ok(_sdr) => println!("   ✓ Successfully opened device with fd 42"),
        Err(e) => println!("   ✗ Failed to open device: {}", e),
    }
    
    println!("\nDemo complete! The new API supports both index and file descriptor based opening.");
}