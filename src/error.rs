// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use std::{error, fmt, result};

/// A result of a function that may return a `Error`.
pub type Result<T> = result::Result<T, RtlsdrError>;

// Macro to create an error enum with From converters for each input error class
macro_rules! define_errcodes {
    [ $typename:ident => $( $name:ident $(: $class:ty)? ),+ ] => {
        #[derive(Debug)]
        pub enum $typename {
            $(
                $name $( ($class) )?,
            )+
        }

        impl fmt::Display for $typename {
            fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                match *self {
                    $(
                        $typename::$name(ref err) => err.fmt(f),
                    )+
                }
            }
        }

        $( $(
            impl From<$class> for $typename {
                fn from(e: $class) -> Self {
                    $typename::$name(e)
                }
            } )?
        )+
    };
}

define_errcodes![
    RtlsdrError =>
    Usb : rusb::Error,
    RtlsdrErr: String
];

impl error::Error for RtlsdrError {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            RtlsdrError::Usb(e) => Some(e),
            RtlsdrError::RtlsdrErr(_) => None,
        }
    }
}
