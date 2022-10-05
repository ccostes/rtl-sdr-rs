use std::{fmt, result};
// use std::error::Error;

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
