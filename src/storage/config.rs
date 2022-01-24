use std::path::PathBuf;

use super::{s3::S3Storage, filesystem::FileSystem};

#[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
#[serde(tag = "type")]
pub enum StorageConfig {
    FileSystem{
        base_path: PathBuf,
    },
    S3{
        bucket: String,
        prefix: String,
    }
}
impl StorageConfig {
    pub fn build_s3(&self) -> S3Storage {
        todo!()
    }

    pub fn build_fs(&self) -> FileSystem {
        todo!()
    }
}