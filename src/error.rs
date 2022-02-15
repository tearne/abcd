use std::{error::Error, num::TryFromIntError};

use aws_sdk_s3::{SdkError, error::GetObjectError};
use aws_smithy_http::operation::Response;

pub type ABCDResult<T> = std::result::Result<T, ABCDError>;

#[derive(Debug)]
pub enum ABCDError {
    Io(std::io::Error),
    Os(std::ffi::OsString),
    Parse(std::num::ParseIntError),
    Serde(serde_json::Error),
    GenAlreadySaved(String),
    WasWorkingOnAnOldGeneration(String),
    NoGenZeroDirExists(String),
    Regex(regex::Error),
    S3GetError(aws_sdk_s3::SdkError<aws_sdk_s3::error::GetObjectError>),
    S3ListError(aws_sdk_s3::SdkError<aws_sdk_s3::error::ListObjectsV2Error>),
    S3PutError(aws_sdk_s3::SdkError<aws_sdk_s3::error::PutObjectError>),
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
            ABCDError::GenAlreadySaved(ref msg) => write!(f, "{}", msg),
            ABCDError::WasWorkingOnAnOldGeneration(ref msg) => write!(f, "{}", msg),
            ABCDError::NoGenZeroDirExists(ref msg) => write!(f, "{}", msg),
            ABCDError::Regex(ref err) => err.fmt(f),
            ABCDError::S3GetError(ref err) => err.fmt(f),
            ABCDError::S3ListError(ref err) => err.fmt(f),
            ABCDError::S3PutError(ref err) => err.fmt(f),
            ABCDError::Other(msg) => f.write_fmt(format_args!("ABCD error: {}", msg)),
            ABCDError::CastError(ref err) => err.fmt(f),
        }
    }
}

impl std::error::Error for ABCDError {}

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

// impl<E: 'static + Error, R = Response> From<SdkError<E, R>> for ABCDError {
//     fn from(value: SdkError<E,R>) -> Self {
//         ABCDError::Other(value.to_string())
//     }
// }

impl From<aws_sdk_s3::SdkError<aws_sdk_s3::error::GetObjectError>> for ABCDError {
    fn from(value: aws_sdk_s3::SdkError<aws_sdk_s3::error::GetObjectError>) -> Self {
        ABCDError::S3GetError(value)
    }
}

impl From<aws_sdk_s3::SdkError<aws_sdk_s3::error::ListObjectsV2Error>> for ABCDError {
    fn from(value: aws_sdk_s3::SdkError<aws_sdk_s3::error::ListObjectsV2Error>) -> Self {
        ABCDError::S3ListError(value)
    }
}

impl From<aws_sdk_s3::SdkError<aws_sdk_s3::error::PutObjectError>> for ABCDError {
    fn from(value: aws_sdk_s3::SdkError<aws_sdk_s3::error::PutObjectError>) -> Self {
        ABCDError::S3PutError(value)
    }
}

impl From<TryFromIntError> for ABCDError {
    fn from(value: TryFromIntError) -> Self {
        ABCDError::CastError(value)
    }
}
