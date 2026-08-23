// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

#![cfg_attr(test, allow(dead_code))]

//! Continuous IQ streaming backed by nusb's endpoint transfer queue.

use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

use nusb::transfer::{Buffer, Bulk, In, TransferError};
use nusb::Endpoint;

use crate::error::{Result, RtlsdrError};
use crate::{RtlSdr, TunerGain, DEFAULT_ASYNC_BUF_NUMBER, DEFAULT_BUF_LENGTH};

const CANCEL_POLL: Duration = Duration::from_millis(100);

/// A configuration change applied to an [`AsyncReadHandle`] stream.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum AsyncReadConfigChange {
    CenterFrequency(u32),
    TunerGain(TunerGain),
    SampleRate(u32),
}

/// An item produced by an owned asynchronous reader.
#[derive(Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum AsyncReadEvent {
    /// IQ bytes captured under the indicated configuration generation.
    Samples { generation: u64, data: Vec<u8> },
    /// An exact stream boundary following a successful configuration change.
    /// All subsequent samples carry this generation until the next change.
    Reconfigured {
        generation: u64,
        change: AsyncReadConfigChange,
    },
}

struct ControlCommand {
    change: AsyncReadConfigChange,
    reply: Sender<Result<u64>>,
}

/// Cloneable control access to an owned asynchronous reader.
#[derive(Clone)]
pub struct AsyncReadControlHandle {
    command_tx: Sender<ControlCommand>,
    stop: Arc<AtomicBool>,
    dropped: Arc<AtomicU64>,
}

impl AsyncReadControlHandle {
    /// Retunes the receiver and returns the new stream generation.
    pub fn set_center_freq(&self, frequency: u32) -> Result<u64> {
        self.send(AsyncReadConfigChange::CenterFrequency(frequency))
    }

    /// Alias for [`Self::set_center_freq`].
    pub fn tune(&self, frequency: u32) -> Result<u64> {
        self.set_center_freq(frequency)
    }

    /// Changes tuner gain and returns the new stream generation.
    pub fn set_tuner_gain(&self, gain: TunerGain) -> Result<u64> {
        self.send(AsyncReadConfigChange::TunerGain(gain))
    }

    /// Changes the sample rate and returns the new stream generation.
    pub fn set_sample_rate(&self, sample_rate: u32) -> Result<u64> {
        self.send(AsyncReadConfigChange::SampleRate(sample_rate))
    }

    /// Requests streaming shutdown.
    pub fn stop(&self) {
        self.stop.store(true, Ordering::Release);
    }

    /// Number of sample chunks dropped because the consumer queue was full.
    pub fn dropped_chunks(&self) -> u64 {
        self.dropped.load(Ordering::Acquire)
    }

    fn send(&self, change: AsyncReadConfigChange) -> Result<u64> {
        if self.stop.load(Ordering::Acquire) {
            return Err(control_error("async reader has stopped"));
        }

        let (reply, response) = mpsc::channel();
        self.command_tx
            .send(ControlCommand { change, reply })
            .map_err(|_| control_error("async control channel is closed"))?;
        response
            .recv()
            .map_err(|_| control_error("async reader stopped before acknowledging control"))?
    }
}

/// An owned continuous IQ stream.
///
/// Dropping this handle stops the stream and joins its worker thread. Use
/// [`Self::control_handle`] to adjust the device from another thread.
pub struct AsyncReadHandle {
    event_rx: Option<Receiver<Result<AsyncReadEvent>>>,
    queued_samples: Arc<AtomicUsize>,
    control: AsyncReadControlHandle,
    worker: Option<JoinHandle<()>>,
}

impl AsyncReadHandle {
    pub fn control_handle(&self) -> AsyncReadControlHandle {
        self.control.clone()
    }

    pub fn recv(&self) -> Option<Result<AsyncReadEvent>> {
        let event = self.event_rx.as_ref()?.recv().ok()?;
        self.release_sample_slot(&event);
        Some(event)
    }

    pub fn try_recv(&self) -> Option<Result<AsyncReadEvent>> {
        let event = self.event_rx.as_ref()?.try_recv().ok()?;
        self.release_sample_slot(&event);
        Some(event)
    }

    pub fn stop(&self) {
        self.control.stop();
    }

    pub fn dropped_chunks(&self) -> u64 {
        self.control.dropped_chunks()
    }

    fn release_sample_slot(&self, event: &Result<AsyncReadEvent>) {
        if matches!(event, Ok(AsyncReadEvent::Samples { .. })) {
            self.queued_samples.fetch_sub(1, Ordering::AcqRel);
        }
    }
}

impl Iterator for AsyncReadHandle {
    type Item = Result<AsyncReadEvent>;

    fn next(&mut self) -> Option<Self::Item> {
        self.recv()
    }
}

impl Drop for AsyncReadHandle {
    fn drop(&mut self) {
        self.stop();
        // Disconnect before joining so the worker notices that no events can
        // be delivered while it winds down.
        drop(self.event_rx.take());
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

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

pub(crate) fn start_async_reader(
    sdr: RtlSdr,
    buf_num: usize,
    buf_len: usize,
) -> Result<AsyncReadHandle> {
    let (buf_num, buf_len) = normalize_buffer_config(buf_num, buf_len)?;
    let endpoint = sdr.sdr.async_endpoint()?;
    let queue_len = buf_num
        .checked_mul(2)
        .ok_or_else(|| control_error("async buffer count is too large"))?;
    let (event_tx, event_rx) = mpsc::channel();
    let (command_tx, command_rx) = mpsc::channel();
    let stop = Arc::new(AtomicBool::new(false));
    let dropped = Arc::new(AtomicU64::new(0));
    let queued_samples = Arc::new(AtomicUsize::new(0));

    let control = AsyncReadControlHandle {
        command_tx,
        stop: Arc::clone(&stop),
        dropped: Arc::clone(&dropped),
    };
    let worker_queued_samples = Arc::clone(&queued_samples);
    let worker = thread::spawn(move || {
        OwnedReaderWorker {
            sdr,
            endpoint,
            buf_num,
            buf_len,
            queue_len,
            event_tx,
            command_rx,
            stop,
            dropped,
            queued_samples: worker_queued_samples,
        }
        .run()
    });

    Ok(AsyncReadHandle {
        event_rx: Some(event_rx),
        queued_samples,
        control,
        worker: Some(worker),
    })
}

struct OwnedReaderWorker {
    sdr: RtlSdr,
    endpoint: Endpoint<Bulk, In>,
    buf_num: usize,
    buf_len: usize,
    queue_len: usize,
    event_tx: Sender<Result<AsyncReadEvent>>,
    command_rx: Receiver<ControlCommand>,
    stop: Arc<AtomicBool>,
    dropped: Arc<AtomicU64>,
    queued_samples: Arc<AtomicUsize>,
}

impl OwnedReaderWorker {
    fn run(mut self) {
        for _ in 0..self.buf_num {
            self.endpoint.submit(Buffer::new(self.buf_len));
        }

        let mut generation = 0;
        while self.endpoint.pending() > 0 && !self.stop.load(Ordering::Acquire) {
            match self.command_rx.try_recv() {
                Ok(command) => {
                    let buffers = cancel_and_collect(&mut self.endpoint);
                    match apply_change(&mut self.sdr, &command.change) {
                        Ok(()) => {
                            if let Err(error) = self.sdr.reset_buffer() {
                                let message = format!(
                                    "failed to reset buffer after reconfiguration: {error}"
                                );
                                let _ = command.reply.send(Err(control_error(&message)));
                                break;
                            }

                            generation += 1;
                            let _ = command.reply.send(Ok(generation));
                            let event = AsyncReadEvent::Reconfigured {
                                generation,
                                change: command.change,
                            };
                            if self.event_tx.send(Ok(event)).is_err() {
                                break;
                            }
                        }
                        Err(error) => {
                            let _ = command.reply.send(Err(error));
                        }
                    }

                    if self.stop.load(Ordering::Acquire) {
                        break;
                    }
                    for buffer in buffers {
                        self.endpoint.submit(buffer);
                    }
                    continue;
                }
                Err(TryRecvError::Disconnected | TryRecvError::Empty) => {}
            }

            let Some(completion) = self.endpoint.wait_next_complete(CANCEL_POLL) else {
                continue;
            };

            if let Err(error) = completion.status {
                self.endpoint.cancel_all();
                drain(&mut self.endpoint);
                let _ = self
                    .event_tx
                    .send(Err(RtlsdrError::from_usb_transfer(error)));
                break;
            }

            if completion.actual_len > 0 {
                if reserve_sample_slot(&self.queued_samples, self.queue_len) {
                    let event = AsyncReadEvent::Samples {
                        generation,
                        data: completion.buffer[..completion.actual_len].to_vec(),
                    };
                    if self.event_tx.send(Ok(event)).is_err() {
                        self.queued_samples.fetch_sub(1, Ordering::AcqRel);
                        break;
                    }
                } else {
                    self.dropped.fetch_add(1, Ordering::AcqRel);
                }
            }
            self.endpoint.submit(completion.buffer);
        }

        self.endpoint.cancel_all();
        drain(&mut self.endpoint);
        drop(self.endpoint);
        self.stop.store(true, Ordering::Release);
        let _ = self.sdr.close();
    }
}

fn reserve_sample_slot(queued: &AtomicUsize, limit: usize) -> bool {
    queued
        .fetch_update(Ordering::AcqRel, Ordering::Acquire, |count| {
            (count < limit).then_some(count + 1)
        })
        .is_ok()
}

fn apply_change(sdr: &mut RtlSdr, change: &AsyncReadConfigChange) -> Result<()> {
    match change {
        AsyncReadConfigChange::CenterFrequency(frequency) => sdr.set_center_freq(*frequency),
        AsyncReadConfigChange::TunerGain(gain) => sdr.set_tuner_gain(gain.clone()),
        AsyncReadConfigChange::SampleRate(sample_rate) => sdr.set_sample_rate(*sample_rate),
    }
}

fn cancel_and_collect(endpoint: &mut Endpoint<Bulk, In>) -> Vec<Buffer> {
    endpoint.cancel_all();
    let mut buffers = Vec::with_capacity(endpoint.pending());
    while endpoint.pending() > 0 {
        if let Some(completion) = endpoint.wait_next_complete(CANCEL_POLL) {
            buffers.push(completion.buffer);
        }
    }
    buffers
}

fn normalize_buffer_config(buf_num: usize, buf_len: usize) -> Result<(usize, usize)> {
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
        return Err(control_error(&format!(
            "Invalid async buffer length {buf_len} (must be multiple of 512)"
        )));
    }
    Ok((buf_num, buf_len))
}

fn control_error(message: &str) -> RtlsdrError {
    RtlsdrError::RtlsdrErr(message.to_string())
}

fn drain(endpoint: &mut Endpoint<Bulk, In>) {
    while endpoint.pending() > 0 {
        let _ = endpoint.wait_next_complete(CANCEL_POLL);
    }
}

#[cfg(test)]
mod tests {
    use super::{
        normalize_buffer_config, reserve_sample_slot, AsyncReadConfigChange,
        AsyncReadControlHandle, CancelHandle,
    };
    use crate::{DEFAULT_ASYNC_BUF_NUMBER, DEFAULT_BUF_LENGTH};
    use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
    use std::sync::{mpsc, Arc};
    use std::thread;

    #[test]
    fn cancellation_is_shared_and_sticky() {
        let cancel = CancelHandle::new();
        let clone = cancel.clone();

        assert!(!cancel.is_cancelled());
        clone.cancel();
        assert!(cancel.is_cancelled());
        assert!(clone.is_cancelled());
    }

    #[test]
    fn owned_reader_buffer_config_uses_defaults_and_validates_alignment() {
        assert_eq!(
            normalize_buffer_config(0, 0).unwrap(),
            (DEFAULT_ASYNC_BUF_NUMBER, DEFAULT_BUF_LENGTH)
        );
        assert!(normalize_buffer_config(1, 511).is_err());
        assert_eq!(normalize_buffer_config(3, 1024).unwrap(), (3, 1024));
    }

    #[test]
    fn control_calls_wait_for_the_worker_acknowledgement() {
        let (command_tx, command_rx) = mpsc::channel();
        let control = AsyncReadControlHandle {
            command_tx,
            stop: Arc::new(AtomicBool::new(false)),
            dropped: Arc::new(AtomicU64::new(0)),
        };

        let caller = thread::spawn(move || control.set_center_freq(101_100_000));
        let command = command_rx.recv().unwrap();
        assert_eq!(
            command.change,
            AsyncReadConfigChange::CenterFrequency(101_100_000)
        );
        command.reply.send(Ok(7)).unwrap();
        assert_eq!(caller.join().unwrap().unwrap(), 7);
    }

    #[test]
    fn stopped_control_handle_rejects_new_commands() {
        let (command_tx, _command_rx) = mpsc::channel();
        let control = AsyncReadControlHandle {
            command_tx,
            stop: Arc::new(AtomicBool::new(true)),
            dropped: Arc::new(AtomicU64::new(0)),
        };

        assert!(control.set_sample_rate(1_920_000).is_err());
    }

    #[test]
    fn sample_queue_reservation_stops_at_the_limit() {
        let queued = AtomicUsize::new(0);
        assert!(reserve_sample_slot(&queued, 2));
        assert!(reserve_sample_slot(&queued, 2));
        assert!(!reserve_sample_slot(&queued, 2));
        assert_eq!(queued.load(Ordering::Acquire), 2);
    }
}
