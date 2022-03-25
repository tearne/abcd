use std::num::TryFromIntError;

pub type ABCDResult<T> = std::result::Result<T, ABCDError>;

#[derive(Debug)]
pub enum ABCDError {
    AlgortihmError(String),
    Configuration(String),
    Io(std::io::Error),
    Os(std::ffi::OsString),
    Parse(std::num::ParseIntError),
    SerdeError(String),
    GenAlreadySaved(String),
    StorageInitError,
    StorageConsistencyError(String),
    WasWorkingOnAnOldGeneration(String),
    Regex(regex::Error),
    S3OperationError(String),
    Other(String),
    CastError(TryFromIntError),
}

impl std::fmt::Display for ABCDError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ABCDError::AlgortihmError(ref msg) => write!(f, "{}", msg),
            ABCDError::Configuration(ref msg) => write!(f, "{}", msg),
            ABCDError::Io(ref err) => err.fmt(f),
            ABCDError::Os(ref original) => {
                f.write_fmt(format_args!("Failed to convert to string: {:?}", original))
            }
            ABCDError::Parse(ref err) => err.fmt(f),
            ABCDError::SerdeError(ref msg) => write!(f, "{}", msg),
            ABCDError::GenAlreadySaved(ref msg) => write!(f, "{}", msg),
            ABCDError::WasWorkingOnAnOldGeneration(ref msg) => write!(f, "{}", msg),
            ABCDError::StorageInitError => write!(f, "Storage init error"),
            ABCDError::StorageConsistencyError(ref msg) => write!(f, "{}", msg),
            ABCDError::Regex(ref err) => err.fmt(f),
            ABCDError::S3OperationError(ref msg) => write!(f, "{}", msg),
            ABCDError::Other(msg) => f.write_fmt(format_args!("ABCD error: {}", msg)),
            ABCDError::CastError(ref err) => err.fmt(f),
        }
    }
}

impl std::error::Error for ABCDError {}

impl From<serde_json::Error> for ABCDError {
    fn from(value: serde_json::Error) -> Self {
        ABCDError::SerdeError(format!("{}", value))
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

impl From<aws_sdk_s3::SdkError<aws_sdk_s3::error::GetObjectError>> for ABCDError {
    fn from(value: aws_sdk_s3::SdkError<aws_sdk_s3::error::GetObjectError>) -> Self {
        ABCDError::S3OperationError(format!("Get object error: {}", value))
    }
}

impl From<aws_sdk_s3::SdkError<aws_sdk_s3::error::ListObjectsV2Error>> for ABCDError {
    fn from(value: aws_sdk_s3::SdkError<aws_sdk_s3::error::ListObjectsV2Error>) -> Self {
        ABCDError::S3OperationError(format!("List object error: {}", value))
    }
}

impl From<aws_sdk_s3::SdkError<aws_sdk_s3::error::PutObjectError>> for ABCDError {
    fn from(value: aws_sdk_s3::SdkError<aws_sdk_s3::error::PutObjectError>) -> Self {
        ABCDError::S3OperationError(format!("Put object error: {}", value))
    }
}

impl From<aws_sdk_s3::SdkError<aws_sdk_s3::error::GetBucketVersioningError>> for ABCDError {
    fn from(value: aws_sdk_s3::SdkError<aws_sdk_s3::error::GetBucketVersioningError>) -> Self {
        ABCDError::S3OperationError(format!("Failed to get bucket version data: {}", value))
    }
}

impl From<aws_sdk_s3::SdkError<aws_sdk_s3::error::ListObjectVersionsError>> for ABCDError {
    fn from(value: aws_sdk_s3::SdkError<aws_sdk_s3::error::ListObjectVersionsError>) -> Self {
        ABCDError::S3OperationError(format!("Failed to list object version data: {}", value))
    }
}

impl From<aws_sdk_s3::SdkError<aws_sdk_s3::error::DeleteObjectsError>> for ABCDError {
    fn from(value: aws_sdk_s3::SdkError<aws_sdk_s3::error::DeleteObjectsError>) -> Self {
        ABCDError::S3OperationError(format!("Failed to delete objects: {}", value))
    }
}

impl From<aws_smithy_http::byte_stream::Error> for ABCDError {
    fn from(value: aws_smithy_http::byte_stream::Error) -> Self {
        ABCDError::S3OperationError(format!("Byte stream error: {}", value))
    }
}

impl From<TryFromIntError> for ABCDError {
    fn from(value: TryFromIntError) -> Self {
        ABCDError::CastError(value)
    }
}
