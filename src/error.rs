use std::fmt;

pub type ABCDResult<T> = std::result::Result<T, ABCDError>;

#[derive(Debug)]
pub enum ABCDError {
    Io(std::io::Error),
    Os(std::ffi::OsString),
    Parse(std::num::ParseIntError),
    Serde(serde_json::Error),
    GenAlreadySaved(String),
}

impl fmt::Display for ABCDError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ABCDError::Io(ref err) => err.fmt(f),
            ABCDError::Os(ref original) => {
                f.write_fmt(format_args!("Failed to convert to string: {:?}", original))
            }
            ABCDError::Parse(ref err) => err.fmt(f),
            ABCDError::Serde(ref err) => err.fmt(f),
            ABCDError::GenAlreadySaved(ref msg) => write!(f, "{}", msg), //f.write_str(string.as_str()),
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
