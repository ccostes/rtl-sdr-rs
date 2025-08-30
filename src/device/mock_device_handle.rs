// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Mock version of rusb::DeviceHandle
use crate::DeviceId;
use crate::error::Result;
use mockall::mock;

use std::time::Duration;

mock! {
    #[derive(Debug)]
    pub DeviceHandle {
        pub fn open(device_id: DeviceId) -> Result<Self>;
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

    }
}
