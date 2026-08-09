// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

#![cfg_attr(test, allow(dead_code))]

//! Continuous IQ streaming backed by nusb's endpoint transfer queue.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use nusb::transfer::{Buffer, Bulk, In, TransferError};
use nusb::Endpoint;

use crate::error::{Result, RtlsdrError};
use crate::{DEFAULT_ASYNC_BUF_NUMBER, DEFAULT_BUF_LENGTH};

const CANCEL_POLL: Duration = Duration::from_millis(100);

/// One-shot tear-down signal for [`crate::RtlSdr::read_async`].
///
/// Once cancelled, a handle remains cancelled. Create a new handle for each
/// subsequent streaming session.
#[derive(Clone, Default, Debug)]
pub struct CancelHandle {
    flag: Arc<AtomicBool>,
}

impl CancelHandle {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self) {
        self.flag.store(true, Ordering::Release);
    }

    pub fn is_cancelled(&self) -> bool {
        self.flag.load(Ordering::Acquire)
    }
}

pub(crate) fn read_async_blocking<F>(
    endpoint: &mut Endpoint<Bulk, In>,
    buf_num: usize,
    buf_len: usize,
    cancel: &CancelHandle,
    mut callback: F,
) -> Result<()>
where
    F: FnMut(&[u8]),
{
    let buf_num = if buf_num == 0 {
        DEFAULT_ASYNC_BUF_NUMBER
    } else {
        buf_num
    };
    let buf_len = if buf_len == 0 {
        DEFAULT_BUF_LENGTH
    } else {
        buf_len
    };

    if buf_len % 512 != 0 {
        return Err(RtlsdrError::RtlsdrErr(format!(
            "Invalid async buffer length {buf_len} (must be multiple of 512)"
        )));
    }

    if cancel.is_cancelled() {
        return Ok(());
    }

    for _ in 0..buf_num {
        endpoint.submit(Buffer::new(buf_len));
    }

    while endpoint.pending() > 0 {
        if cancel.is_cancelled() {
            endpoint.cancel_all();
        }

        let Some(completion) = endpoint.wait_next_complete(CANCEL_POLL) else {
            continue;
        };

        let should_resubmit = !cancel.is_cancelled() && completion.status.is_ok();
        if completion.actual_len > 0 {
            callback(&completion.buffer[..completion.actual_len]);
        }

        match completion.status {
            Ok(()) if should_resubmit => endpoint.submit(completion.buffer),
            Ok(()) => {}
            Err(TransferError::Cancelled) if cancel.is_cancelled() => {}
            Err(e) => {
                endpoint.cancel_all();
                drain(endpoint);
                return Err(RtlsdrError::from_usb_transfer(e));
            }
        }
    }

    Ok(())
}

fn drain(endpoint: &mut Endpoint<Bulk, In>) {
    while endpoint.pending() > 0 {
        let _ = endpoint.wait_next_complete(CANCEL_POLL);
    }
}

#[cfg(test)]
mod tests {
    use super::CancelHandle;

    #[test]
    fn cancellation_is_shared_and_sticky() {
        let cancel = CancelHandle::new();
        let clone = cancel.clone();

        assert!(!cancel.is_cancelled());
        clone.cancel();
        assert!(cancel.is_cancelled());
        assert!(clone.is_cancelled());
    }
}
