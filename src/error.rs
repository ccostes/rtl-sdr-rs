// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use std::{error, fmt, io, result};

/// A result returned by this crate.
pub type Result<T> = result::Result<T, RtlsdrError>;

/// Stable categories for USB failures.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum UsbErrorKind {
    Disconnected,
    Busy,
    PermissionDenied,
    NotFound,
    Unsupported,
    Cancelled,
    Timeout,
    Stall,
    Fault,
    InvalidArgument,
    Other,
}

#[derive(Debug)]
enum UsbErrorSource {
    Device(nusb::Error),
    Transfer(nusb::transfer::TransferError),
}

/// A USB failure whose public representation is independent of the USB backend.
#[derive(Debug)]
pub struct UsbError {
    kind: UsbErrorKind,
    source: UsbErrorSource,
}

impl UsbError {
    pub fn kind(&self) -> UsbErrorKind {
        self.kind
    }

    fn from_device(source: nusb::Error) -> Self {
        let kind = match source.kind() {
            nusb::ErrorKind::Disconnected => UsbErrorKind::Disconnected,
            nusb::ErrorKind::Busy => UsbErrorKind::Busy,
            nusb::ErrorKind::PermissionDenied => UsbErrorKind::PermissionDenied,
            nusb::ErrorKind::NotFound => UsbErrorKind::NotFound,
            nusb::ErrorKind::Unsupported => UsbErrorKind::Unsupported,
            nusb::ErrorKind::Other => UsbErrorKind::Other,
            _ => UsbErrorKind::Other,
        };
        Self {
            kind,
            source: UsbErrorSource::Device(source),
        }
    }

    fn from_transfer(source: nusb::transfer::TransferError) -> Self {
        use nusb::transfer::TransferError;

        let kind = match source {
            TransferError::Cancelled => UsbErrorKind::Cancelled,
            TransferError::Stall => UsbErrorKind::Stall,
            TransferError::Disconnected => UsbErrorKind::Disconnected,
            TransferError::Fault => UsbErrorKind::Fault,
            TransferError::InvalidArgument => UsbErrorKind::InvalidArgument,
            TransferError::Unknown(_) => UsbErrorKind::Other,
        };
        Self {
            kind,
            source: UsbErrorSource::Transfer(source),
        }
    }

    fn from_timed_transfer(source: nusb::transfer::TransferError) -> Self {
        let mut error = Self::from_transfer(source);
        if error.kind == UsbErrorKind::Cancelled {
            error.kind = UsbErrorKind::Timeout;
        }
        error
    }
}

impl fmt::Display for UsbError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.kind == UsbErrorKind::Timeout {
            return f.write_str("USB transfer timed out");
        }

        match &self.source {
            UsbErrorSource::Device(source) => source.fmt(f),
            UsbErrorSource::Transfer(source) => source.fmt(f),
        }
    }
}

impl error::Error for UsbError {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match &self.source {
            UsbErrorSource::Device(source) => Some(source),
            UsbErrorSource::Transfer(source) => Some(source),
        }
    }
}

#[derive(Debug)]
#[non_exhaustive]
pub enum RtlsdrError {
    Usb(UsbError),
    Io(io::Error),
    RtlsdrErr(String),
}

impl RtlsdrError {
    pub(crate) fn from_usb(source: nusb::Error) -> Self {
        Self::Usb(UsbError::from_device(source))
    }

    pub(crate) fn from_usb_transfer(source: nusb::transfer::TransferError) -> Self {
        Self::Usb(UsbError::from_transfer(source))
    }

    pub(crate) fn from_timed_usb_transfer(source: nusb::transfer::TransferError) -> Self {
        Self::Usb(UsbError::from_timed_transfer(source))
    }
}

impl From<UsbError> for RtlsdrError {
    fn from(error: UsbError) -> Self {
        Self::Usb(error)
    }
}

impl From<io::Error> for RtlsdrError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<String> for RtlsdrError {
    fn from(error: String) -> Self {
        Self::RtlsdrErr(error)
    }
}

impl fmt::Display for RtlsdrError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Usb(error) => error.fmt(f),
            Self::Io(error) => error.fmt(f),
            Self::RtlsdrErr(error) => error.fmt(f),
        }
    }
}

impl error::Error for RtlsdrError {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Self::Usb(error) => Some(error),
            Self::Io(error) => Some(error),
            Self::RtlsdrErr(_) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{RtlsdrError, UsbErrorKind};
    use nusb::transfer::TransferError;

    #[test]
    fn transfer_errors_map_to_stable_usb_kinds() {
        let cases = [
            (TransferError::Cancelled, UsbErrorKind::Cancelled),
            (TransferError::Stall, UsbErrorKind::Stall),
            (TransferError::Disconnected, UsbErrorKind::Disconnected),
            (TransferError::Fault, UsbErrorKind::Fault),
            (
                TransferError::InvalidArgument,
                UsbErrorKind::InvalidArgument,
            ),
            (TransferError::Unknown(1), UsbErrorKind::Other),
        ];

        for (source, expected) in cases {
            let error = RtlsdrError::from_usb_transfer(source);
            let RtlsdrError::Usb(error) = error else {
                panic!("expected USB error");
            };
            assert_eq!(error.kind(), expected);
            assert!(std::error::Error::source(&error).is_some());
        }
    }

    #[test]
    fn cancelled_timed_transfer_maps_to_timeout() {
        let error = RtlsdrError::from_timed_usb_transfer(TransferError::Cancelled);
        let RtlsdrError::Usb(error) = error else {
            panic!("expected USB error");
        };

        assert_eq!(error.kind(), UsbErrorKind::Timeout);
        assert_eq!(error.to_string(), "USB transfer timed out");
        assert!(std::error::Error::source(&error).is_some());
    }
}
