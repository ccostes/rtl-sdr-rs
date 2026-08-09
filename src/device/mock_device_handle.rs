// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Mock version of DeviceHandle
use crate::error::Result;
use crate::DeviceId;
use mockall::mock;

use std::time::Duration;

mock! {
    #[derive(Debug)]
    pub DeviceHandle {
        pub fn open<'a>(device_id: DeviceId<'a>) -> Result<Self>;
        pub fn claim_interface(&mut self, iface: u8) -> Result<()>;
        pub fn reset(&mut self) -> Result<()>;
        pub fn read_control(
            &self,
            request_type: u8,
            request: u8,
            value: u16,
            index: u16,
            buf: &mut [u8],
            timeout: Duration,
        ) -> Result<usize>;
        pub fn write_control(
            &self,
            request_type: u8,
            request: u8,
            value: u16,
            index: u16,
            buf: &[u8],
            timeout: Duration,
        ) -> Result<usize>;
        pub fn read_bulk(
            &self,
            endpoint: u8,
            buf: &mut [u8],
            timeout: Duration,
        ) -> Result<usize>;
        pub fn read_async(
            &self,
            endpoint: u8,
            buf_num: usize,
            buf_len: usize,
            cancel: &crate::async_read::CancelHandle,
            callback: &mut dyn FnMut(&[u8]),
        ) -> Result<()>;
        pub fn bulk_in_endpoint(
            &self,
            endpoint: u8,
        ) -> Result<nusb::Endpoint<nusb::transfer::Bulk, nusb::transfer::In>>;
        pub fn get_usb_strings(
            &self,
        ) -> Result<(Option<String>, Option<String>, Option<String>)>;

    }
}
