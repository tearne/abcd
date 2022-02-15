use std::path::PathBuf;

use aws_sdk_s3::{Region, Client};
use tokio::runtime::Runtime;

use super::{filesystem::FileSystem, s3::S3System};

#[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
#[serde(tag = "type")]
pub enum StorageConfig {
    FileSystem { base_path: PathBuf },
    S3 { bucket: String, prefix: String },
}
impl StorageConfig {
    pub fn build_s3(&self) -> S3System {
        match self {
            StorageConfig::FileSystem { base_path: _ } => panic!("Can't build FileSystem from S3 config"),
            StorageConfig::S3{bucket, prefix} => {
                S3System::new(bucket, prefix)
            }
        }
    }

    pub fn build_fs(&self) -> FileSystem {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::StorageConfig;
    use crate::test_helper::local_test_file_path;

    #[test]
    fn build_s3_storage_properties_from_config() {
        let path = local_test_file_path("resources/test/config_test.toml");
        let config: StorageConfig = toml::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        let storage = config.build_s3();

        assert_eq!("myBucket", storage.bucket);
        assert_eq!("myPrefix", storage.prefix);
    }
}
