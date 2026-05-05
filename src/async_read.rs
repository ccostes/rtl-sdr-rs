// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Asynchronous IQ streaming via libusb's bulk-transfer async API.
//!
//! Mirrors librtlsdr's `rtlsdr_read_async` / `rtlsdr_cancel_async`:
//! a ring of `buf_num` libusb transfers stays submitted at all times
//! so the device's internal FIFO never drains between USB pipeline
//! refills. Each completed transfer fires the caller's closure with
//! the freshly-received bytes and is then re-submitted automatically.
//!
//! Compared with calling [`crate::RtlSdr::read_sync`] in a tight loop
//! this eliminates the per-iteration USB pipeline gap (a few hundred
//! microseconds to a few milliseconds, depending on the host
//! scheduler) where the host has nothing in flight and the RTL-SDR's
//! ~64 KB on-chip FIFO can overflow into silent sample drops. The
//! visible symptom of those drops in downstream demodulators is a
//! sample-rate undershoot (a few percent low) plus periodic phase
//! discontinuities that knock narrowband PLLs out of lock.
//!
//! The event loop runs synchronously on the calling thread; the user
//! callback also runs there (libusb serialises transfer callbacks on
//! whichever thread is pumping events for that context). Use a
//! [`CancelHandle`] cloned to another thread to tear the loop down
//! cleanly.

use std::os::raw::{c_int, c_uchar, c_uint, c_void};
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicUsize, Ordering};
use std::sync::Arc;

use libusb1_sys::constants::{
    LIBUSB_TRANSFER_COMPLETED, LIBUSB_TRANSFER_ERROR, LIBUSB_TRANSFER_NO_DEVICE,
    LIBUSB_TRANSFER_OVERFLOW, LIBUSB_TRANSFER_STALL, LIBUSB_TRANSFER_TIMED_OUT,
};
use libusb1_sys::{
    libusb_alloc_transfer, libusb_cancel_transfer, libusb_context, libusb_device_handle,
    libusb_fill_bulk_transfer, libusb_free_transfer, libusb_handle_events_timeout_completed,
    libusb_submit_transfer, libusb_transfer,
};

use crate::error::{Result, RtlsdrError::RtlsdrErr};

/// RTL-SDR's bulk-IN endpoint (matches `rtlsdr_read_sync` in
/// librtlsdr and our own `bulk_transfer`).
const BULK_ENDPOINT: u8 = 0x81;

/// Per-transfer libusb timeout. `0` means "no timeout" — cancellation
/// is the only way out, which matches librtlsdr's behaviour. With a
/// finite timeout the FIFO would gap on every per-URB timer fire.
const TRANSFER_TIMEOUT_MS: c_uint = 0;

/// Event-loop poll interval. We re-check the cancel flag this often
/// even if libusb has nothing to deliver, so external cancellation
/// from a UI thread becomes effective in at most this much time.
const POLL_TIMEOUT_USEC: i64 = 100_000; // 100 ms

/// Tear-down signal for [`crate::RtlSdr::read_async`]. Cloning shares
/// the underlying flag — pass a clone to another thread (signal
/// handler, GUI shutdown handler, …) and call [`Self::cancel`] from
/// there to make the streaming loop return.
///
/// `read_async` resets the flag on entry, so re-using the same handle
/// for sequential streaming sessions is safe.
#[derive(Clone, Default, Debug)]
pub struct CancelHandle {
    flag: Arc<AtomicBool>,
}

impl CancelHandle {
    pub fn new() -> Self {
        Self::default()
    }

    /// Asks the matching `read_async` call to return. The actual
    /// return is delayed by at most one poll interval (~100 ms) plus
    /// however long it takes for the in-flight transfers' callbacks
    /// to fire.
    pub fn cancel(&self) {
        self.flag.store(true, Ordering::Release);
    }

    pub fn is_cancelled(&self) -> bool {
        self.flag.load(Ordering::Acquire)
    }
}

/// State shared across all transfers in one async-read session. Lives
/// on the stack of `read_async_blocking` for the duration of the call;
/// transfer callbacks dereference it via raw pointer (the callback
/// signature is `extern "system" fn(*mut libusb_transfer)` so we have
/// no choice).
///
/// SAFETY: libusb fires per-context transfer callbacks serially on
/// the thread that is currently pumping events, which is the same
/// thread that called `read_async_blocking`. There is therefore no
/// concurrent mutation of `Shared::callback`.
struct Shared<F> {
    callback: F,
    in_flight: AtomicUsize,
    cancel: Arc<AtomicBool>,
    /// First non-zero libusb status / error code seen across any
    /// transfer or event-loop call. Reported back to the caller.
    fatal: AtomicI32,
}

extern "system" fn transfer_done<F>(transfer: *mut libusb_transfer)
where
    F: FnMut(&[u8]),
{
    // SAFETY: libusb only invokes us with a valid transfer pointer
    // that has its `user_data` set to `&mut Shared<F>` we stashed in
    // `read_async_blocking`. The Shared struct is alive for the
    // entire duration of that call; we always reach in-flight = 0
    // before the function returns and frees memory.
    unsafe {
        let t = &mut *transfer;
        let shared = &mut *(t.user_data as *mut Shared<F>);
        let status = t.status;

        if status == LIBUSB_TRANSFER_COMPLETED && t.actual_length > 0 {
            let data =
                std::slice::from_raw_parts(t.buffer as *const u8, t.actual_length as usize);
            (shared.callback)(data);
        }

        let user_cancelled = shared.cancel.load(Ordering::Acquire);
        let resubmittable = matches!(
            status,
            LIBUSB_TRANSFER_COMPLETED | LIBUSB_TRANSFER_TIMED_OUT | LIBUSB_TRANSFER_STALL
        );

        if user_cancelled || !resubmittable {
            // Latch the first hard failure so the caller sees it. We
            // intentionally don't latch CANCELLED (= clean shutdown).
            if matches!(
                status,
                LIBUSB_TRANSFER_ERROR | LIBUSB_TRANSFER_NO_DEVICE | LIBUSB_TRANSFER_OVERFLOW
            ) {
                let _ = shared.fatal.compare_exchange(
                    0,
                    status,
                    Ordering::AcqRel,
                    Ordering::Acquire,
                );
            }
            shared.in_flight.fetch_sub(1, Ordering::AcqRel);
            return;
        }

        // Re-submit; on submission failure, fail this transfer and
        // signal the loop so its peers also start tearing down.
        let r = libusb_submit_transfer(transfer);
        if r != 0 {
            let _ = shared
                .fatal
                .compare_exchange(0, r, Ordering::AcqRel, Ordering::Acquire);
            shared.cancel.store(true, Ordering::Release);
            shared.in_flight.fetch_sub(1, Ordering::AcqRel);
        }
    }
}

/// Drive a libusb async streaming loop. Blocks until `cancel` is
/// signalled or every transfer reaches a terminal libusb state.
///
/// `callback` is invoked synchronously on the calling thread once per
/// completed bulk transfer with the bytes actually received (i.e. a
/// view into the URB buffer of length `actual_length`).
///
/// Recommended sizing tracks librtlsdr's defaults: `buf_num = 15` and
/// `buf_len = 16 * 16384 = 262_144` (≈ 4 MB ring) is plenty for
/// 1.92–2.4 MS/s. Smaller `buf_num` (e.g. 4) works on lighter loads
/// but leaves less margin against scheduler hiccups.
///
/// # Safety
/// Caller guarantees:
/// - `handle` is a live, claimed `*mut libusb_device_handle` and no
///   other thread is calling `libusb_handle_events*` on `context`
///   for the duration of this call.
/// - `context` is the matching libusb context for `handle`, and the
///   enclosing rusb `Context` outlives the call.
pub(crate) unsafe fn read_async_blocking<F>(
    handle: *mut libusb_device_handle,
    context: *mut libusb_context,
    buf_num: usize,
    buf_len: usize,
    cancel: &CancelHandle,
    callback: F,
) -> Result<()>
where
    F: FnMut(&[u8]) + Send,
{
    if buf_num == 0 || buf_len == 0 {
        return Err(RtlsdrErr(format!(
            "read_async: buf_num and buf_len must be > 0 (got {}, {})",
            buf_num, buf_len
        )));
    }
    if buf_len > c_int::MAX as usize {
        return Err(RtlsdrErr(format!(
            "read_async: buf_len {} exceeds c_int range ({})",
            buf_len,
            c_int::MAX
        )));
    }

    // Forget any cancellation from a previous run; the caller is
    // explicitly opting in to a new streaming session.
    cancel.flag.store(false, Ordering::Release);

    let mut shared = Shared {
        callback,
        in_flight: AtomicUsize::new(0),
        cancel: cancel.flag.clone(),
        fatal: AtomicI32::new(0),
    };
    let shared_ptr: *mut Shared<F> = &mut shared;

    // Buffers and transfers are owned in parallel `Vec`s. The buffers
    // need a stable address through the call (libusb keeps the raw
    // pointer we hand it inside `libusb_fill_bulk_transfer`), which
    // `Box<[u8]>` provides — pushing the box into a `Vec` moves the
    // box value but leaves the heap allocation in place.
    let mut buffers: Vec<Box<[u8]>> = Vec::with_capacity(buf_num);
    let mut transfers: Vec<*mut libusb_transfer> = Vec::with_capacity(buf_num);

    for _ in 0..buf_num {
        let xfer = libusb_alloc_transfer(0);
        if xfer.is_null() {
            for &t in &transfers {
                libusb_free_transfer(t);
            }
            return Err(RtlsdrErr(
                "libusb_alloc_transfer returned null".to_string(),
            ));
        }
        let mut buf = vec![0u8; buf_len].into_boxed_slice();
        libusb_fill_bulk_transfer(
            xfer,
            handle,
            BULK_ENDPOINT,
            buf.as_mut_ptr() as *mut c_uchar,
            buf_len as c_int,
            transfer_done::<F>,
            shared_ptr as *mut c_void,
            TRANSFER_TIMEOUT_MS,
        );
        buffers.push(buf);
        transfers.push(xfer);
    }

    // Submit all of them. Once we begin, callbacks may fire on this
    // thread the moment we next pump events — they read `shared`
    // through the raw pointer we set up above.
    for &xfer in &transfers {
        shared.in_flight.fetch_add(1, Ordering::AcqRel);
        let r = libusb_submit_transfer(xfer);
        if r != 0 {
            // Roll back this URB's optimistic increment.
            shared.in_flight.fetch_sub(1, Ordering::AcqRel);
            let _ = shared
                .fatal
                .compare_exchange(0, r, Ordering::AcqRel, Ordering::Acquire);
            shared.cancel.store(true, Ordering::Release);
            // Cancel any peers that did manage to submit so we can
            // drain them cleanly before returning.
            for &t in &transfers {
                libusb_cancel_transfer(t);
            }
            drain_in_flight(context, &shared.in_flight);
            for &t in &transfers {
                libusb_free_transfer(t);
            }
            drop(buffers);
            return Err(RtlsdrErr(format!("libusb_submit_transfer: {}", r)));
        }
    }

    // Pump events. We poll cancellation between event-loop iterations
    // so a `CancelHandle::cancel()` from another thread becomes
    // effective within `POLL_TIMEOUT_USEC` even when the device is
    // briefly silent.
    let mut requested_cancel = false;
    while shared.in_flight.load(Ordering::Acquire) > 0 {
        if !requested_cancel && shared.cancel.load(Ordering::Acquire) {
            for &t in &transfers {
                libusb_cancel_transfer(t);
            }
            requested_cancel = true;
        }

        let mut tv = libc::timeval {
            tv_sec: 0,
            tv_usec: POLL_TIMEOUT_USEC as libc::suseconds_t,
        };
        let mut completed: c_int = 0;
        let r = libusb_handle_events_timeout_completed(
            context,
            &mut tv as *mut libc::timeval,
            &mut completed as *mut c_int,
        );
        if r != 0 {
            // libusb error from the event loop itself; treat as
            // fatal, request cancellation, and let the in-flight
            // transfers drain via their callbacks below.
            let _ = shared
                .fatal
                .compare_exchange(0, r, Ordering::AcqRel, Ordering::Acquire);
            if !requested_cancel {
                shared.cancel.store(true, Ordering::Release);
            }
        }
    }

    for &t in &transfers {
        libusb_free_transfer(t);
    }
    drop(buffers);

    let f = shared.fatal.load(Ordering::Acquire);
    if f != 0 {
        return Err(RtlsdrErr(format!(
            "libusb error during async read (status {})",
            f
        )));
    }
    Ok(())
}

/// Pump events until every outstanding transfer has fired its final
/// callback. Used during error-path teardown so we never free a
/// transfer that's still owned by the kernel.
unsafe fn drain_in_flight(context: *mut libusb_context, in_flight: &AtomicUsize) {
    while in_flight.load(Ordering::Acquire) > 0 {
        let mut tv = libc::timeval {
            tv_sec: 0,
            tv_usec: POLL_TIMEOUT_USEC as libc::suseconds_t,
        };
        let mut completed: c_int = 0;
        let _ = libusb_handle_events_timeout_completed(
            context,
            &mut tv as *mut libc::timeval,
            &mut completed as *mut c_int,
        );
    }
}
