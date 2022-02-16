use std::{path::PathBuf, borrow::Cow};

use aws_sdk_s3::{Region, Client};
use envmnt::{ExpandOptions, ExpansionType};
use tokio::runtime::Runtime;

use crate::error::ABCDResult;

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
                // Expand bucket environment variable if appropriate
                let mut options = ExpandOptions::new();
                options.expansion_type = Some(ExpansionType::Unix);
                let bucket = envmnt::expand(bucket, Some(options));
                S3System::new(bucket, prefix.clone())
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
    fn build_s3_storage_properties_from_config_expanding_env_var() {
        let c = StorageConfig::S3 {
            bucket: "a-bucket".into(),
            prefix: "a-prefix".into(),
        };
        println!("===== {}", &toml::to_string_pretty(&c).unwrap());



        envmnt::set("ABCDBucket", "env-var-bucket");

        let path = local_test_file_path("resources/test/config_test.toml");
        let string = std::fs::read_to_string(&path).unwrap();
        println!("----- {}", &string);
        let config: StorageConfig = toml::from_str(&string).unwrap();
        let storage = config.build_s3();

        assert_eq!("s3://env-var-bucket", storage.bucket);
        assert_eq!("myPrefix", storage.prefix);
    }

    // #[test]
    // fn expand_bucket_if_env_var() {
    //     envmnt::set("ABCDBucket", "env-var-bucket");

    //     let path = local_test_file_path("resources/test/config_test.toml");
    //     let config = Config::from_path(path);

    //     let bucket = match config.storage {
    //         crate::storage::config::StorageConfig::FileSystem { base_path:_ } => panic!("expected S3 config"),
    //         crate::storage::config::StorageConfig::S3 { bucket, prefix:_ } => bucket,
    //     };

    //     assert_eq!("s3://env-var-bucket", bucket);
    // }
}
