use std::{num::TryFromIntError, fmt::Display};

pub type ABCDResult<T> = Result<T, ABCDErr>;

#[derive(Debug)]
pub enum ABCDErr {
    /// Returned when found we were working on an out of date previous generation
    StaleGenerationErr(String), //TODO make impossible for clients to see this, somehow?
    
    ParticleErr(String),        //TODO make impossible for clients to see this, somehow?
    TooManyRetriesError(String, Vec<String>),
    InfrastructureError(String),
    SystemError(String),
}

impl Display for ABCDErr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::StaleGenerationErr(ref msg) => write!(f, "GenerationErr: {}", msg),
            Self::ParticleErr(ref msg) => write!(f, "ParticleErr:{}", msg),
            Self::InfrastructureError(ref msg) => write!(f, "InfrastructureErr: {}", msg),
            Self::SystemError(ref msg) => write!(f, "SystemErr: {}", msg),
            Self::TooManyRetriesError(ref msg, ref history) => write!(f, "{}\n  {:#?}", msg, history),
        }
    }
}

impl std::error::Error for ABCDErr {}

impl From<serde_json::Error> for ABCDErr {
    fn from(value: serde_json::Error) -> Self {
        ABCDErr::InfrastructureError(format!("Serde Error: {}", value))
    }
}

impl From<std::io::Error> for ABCDErr {
    fn from(value: std::io::Error) -> Self {
        ABCDErr::InfrastructureError(format!("IO Error: {}", value))
    }
}

impl From<std::ffi::OsString> for ABCDErr {
    fn from(value: std::ffi::OsString) -> Self {
        ABCDErr::InfrastructureError(format!("OS String Error: {:?}", value))
    }
}

impl From<std::num::ParseIntError> for ABCDErr {
    fn from(value: std::num::ParseIntError) -> Self {
        ABCDErr::InfrastructureError(format!("Parse Int Error: {}", value))
    }
}

impl From<regex::Error> for ABCDErr {
    fn from(value: regex::Error) -> Self {
        ABCDErr::InfrastructureError(format!("RegEx Error: {}", value))
    }
}

impl From<aws_sdk_s3::types::SdkError<aws_sdk_s3::error::GetObjectError>> for ABCDErr {
    fn from(value: aws_sdk_s3::types::SdkError<aws_sdk_s3::error::GetObjectError>) -> Self {
        ABCDErr::InfrastructureError(format!("S3 get object error: {}", value))
    }
}

impl From<aws_sdk_s3::types::SdkError<aws_sdk_s3::error::ListObjectsV2Error>> for ABCDErr {
    fn from(value: aws_sdk_s3::types::SdkError<aws_sdk_s3::error::ListObjectsV2Error>) -> Self {
        ABCDErr::InfrastructureError(format!("S3 list object error: {}", value))
    }
}

impl From<aws_sdk_s3::types::SdkError<aws_sdk_s3::error::PutObjectError>> for ABCDErr {
    fn from(value: aws_sdk_s3::types::SdkError<aws_sdk_s3::error::PutObjectError>) -> Self {
        ABCDErr::InfrastructureError(format!("S3 put object error: {}", value))
    }
}

impl From<aws_sdk_s3::types::SdkError<aws_sdk_s3::error::GetBucketVersioningError>> for ABCDErr {
    fn from(value: aws_sdk_s3::types::SdkError<aws_sdk_s3::error::GetBucketVersioningError>) -> Self {
        ABCDErr::InfrastructureError(format!("Failed to get bucket version data: {}", value))
    }
}

impl From<aws_sdk_s3::types::SdkError<aws_sdk_s3::error::ListObjectVersionsError>> for ABCDErr {
    fn from(value: aws_sdk_s3::types::SdkError<aws_sdk_s3::error::ListObjectVersionsError>) -> Self {
        ABCDErr::InfrastructureError(format!("Failed to list object version data: {}", value))
    }
}

impl From<aws_sdk_s3::types::SdkError<aws_sdk_s3::error::DeleteObjectsError>> for ABCDErr {
    fn from(value: aws_sdk_s3::types::SdkError<aws_sdk_s3::error::DeleteObjectsError>) -> Self {
        ABCDErr::InfrastructureError(format!("Failed to delete objects: {}", value))
    }
}

impl From<aws_smithy_http::byte_stream::Error> for ABCDErr {
    fn from(value: aws_smithy_http::byte_stream::Error) -> Self {
        ABCDErr::InfrastructureError(format!("Byte stream error: {}", value))
    }
}

impl From<TryFromIntError> for ABCDErr {
    fn from(value: TryFromIntError) -> Self {
        ABCDErr::InfrastructureError(format!("Cast error: {}", value))
    }
}
