use std::fmt;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
pub enum Error {
    Io(std::io::Error),
    Os(std::ffi::OsString),
    Parse(std::num::ParseIntError),
    Serde(serde_json::Error),
    GenAlreadySaved(String),
}

// impl error::Error for Error {}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            &Error::Io(ref err) => err.fmt(f),
            &Error::Os(ref original) => f.write_fmt(format_args!("Failed to convert to string: {:?}", original)),
            &Error::Parse(ref err) => err.fmt(f),
            &Error::Serde(ref err) => err.fmt(f),
            &Error::GenAlreadySaved(ref msg) => write!(f, "{}", msg),//f.write_str(string.as_str()),
        }
    }
}

impl From<serde_json::Error> for Error {
    fn from(value: serde_json::Error) -> Self {
        Error::Serde(value)
    }
}

impl From<std::io::Error> for Error {
    fn from(value: std::io::Error) -> Self {
        Error::Io(value)
    }
}

impl From<std::ffi::OsString> for Error {
    fn from(value: std::ffi::OsString) -> Self {
        Error::Os(value)
    }
}

impl From<std::num::ParseIntError> for Error {
    fn from(value: std::num::ParseIntError) -> Self {
        Error::Parse(value)
    }
}