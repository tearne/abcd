use std::{error::Error, num::TryFromIntError};

use rusoto_core::RusotoError;

pub type ABCDResult<T> = std::result::Result<T, ABCDError>;

#[derive(Debug)]
pub enum ABCDError {
    Io(std::io::Error),
    Os(std::ffi::OsString),
    Parse(std::num::ParseIntError),
    Serde(serde_json::Error),
    GenAlreadySaved(String),
    Regex(regex::Error),
    RusotoError(String),
    Other(String),
    CastError(TryFromIntError),
}

impl std::fmt::Display for ABCDError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ABCDError::Io(ref err) => err.fmt(f),
            ABCDError::Os(ref original) => {
                f.write_fmt(format_args!("Failed to convert to string: {:?}", original))
            }
            ABCDError::Parse(ref err) => err.fmt(f),
            ABCDError::Serde(ref err) => err.fmt(f),
            ABCDError::GenAlreadySaved(ref msg) => write!(f, "{}", msg), //f.write_str(string.as_str()),
            ABCDError::Regex(ref err) => err.fmt(f),
            ABCDError::RusotoError(msg) => f.write_fmt(format_args!("Rusoto error: {}", msg)),
            ABCDError::Other(msg) => f.write_fmt(format_args!("ABCD error: {}", msg)),
            ABCDError::CastError(ref err) => err.fmt(f),
        }
    }
}

impl From<serde_json::Error> for ABCDError {
    fn from(value: serde_json::Error) -> Self {
        ABCDError::Serde(value)
    }
}

impl From<std::io::Error> for ABCDError {
    fn from(value: std::io::Error) -> Self {
        ABCDError::Io(value)
    }
}

impl From<std::ffi::OsString> for ABCDError {
    fn from(value: std::ffi::OsString) -> Self {
        ABCDError::Os(value)
    }
}

impl From<std::num::ParseIntError> for ABCDError {
    fn from(value: std::num::ParseIntError) -> Self {
        ABCDError::Parse(value)
    }
}

impl From<regex::Error> for ABCDError {
    fn from(value: regex::Error) -> Self {
        ABCDError::Regex(value)
    }
}

impl<T: 'static + Error> From<RusotoError<T>> for ABCDError {
    fn from(value: RusotoError<T>) -> Self {
        ABCDError::Other(value.to_string())
    }
}

impl From<TryFromIntError> for ABCDError {
    fn from(value: TryFromIntError) -> Self {
        ABCDError::CastError(value)
    }
}