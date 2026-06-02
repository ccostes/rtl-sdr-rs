//! Continuous async-streaming IQ capture: prints sample-rate
//! statistics every second to stderr, demonstrating gapless
//! streaming via `read_async`. Press Ctrl-C to stop.
//!
//! Compare with a `read_sync`-loop equivalent: at 1.92 MS/s on a
//! lightly loaded host, `read_async` keeps the throughput within
//! 0.05 % of nominal where `read_sync` typically loses 1–2 % to
//! gaps in the USB pipeline between transfers.
//!
//! Run with `cargo run --example async_capture`.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use rtl_sdr_rs::{CancelHandle, DeviceId, RtlSdr};

const FREQUENCY: u32 = 100_000_000;
const SAMPLE_RATE: u32 = 1_920_000;
const RTL_INDEX: usize = 0;

// Match librtlsdr's default ring (15 × 16 KB ≈ 240 KB). For a more
// generous margin against scheduler hiccups bump these.
const BUF_NUM: usize = 15;
const BUF_LEN: usize = 16 * 16384;

fn main() {
    stderrlog::new().verbosity(log::Level::Info).init().unwrap();

    let cancel = CancelHandle::new();
    let cancel_for_signal = cancel.clone();
    ctrlc::set_handler(move || {
        eprintln!("\nctrl-c received; cancelling…");
        cancel_for_signal.cancel();
    })
    .unwrap();

    let mut sdr = RtlSdr::open(DeviceId::Index(RTL_INDEX)).expect("open");
    sdr.set_sample_rate(SAMPLE_RATE).unwrap();
    sdr.set_center_freq(FREQUENCY).unwrap();
    sdr.reset_buffer().unwrap();

    let bytes_total = Arc::new(AtomicU64::new(0));
    let bytes_for_stats = Arc::clone(&bytes_total);
    let cancel_for_stats = cancel.clone();
    let stats_handle = thread::spawn(move || {
        let mut prev = 0u64;
        let start = Instant::now();
        while !cancel_for_stats.is_cancelled() {
            thread::sleep(Duration::from_secs(1));
            let now = bytes_for_stats.load(Ordering::Acquire);
            let delta = now - prev;
            prev = now;
            eprintln!(
                "{:>4}.0 s: {:.3} MS/s  (cum {:.1} MB)",
                start.elapsed().as_secs(),
                (delta / 2) as f64 / 1e6,
                now as f64 / 1e6,
            );
        }
    });

    let bytes_for_callback = Arc::clone(&bytes_total);
    let result = sdr.read_async(BUF_NUM, BUF_LEN, &cancel, move |buf| {
        bytes_for_callback.fetch_add(buf.len() as u64, Ordering::AcqRel);
    });

    cancel.cancel(); // wake the stats thread if read_async returned on its own
    let _ = stats_handle.join();

    if let Err(e) = result {
        eprintln!("read_async returned error: {e:?}");
    }
}
