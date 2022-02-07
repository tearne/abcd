use std::path::PathBuf;

use super::{filesystem::FileSystem, s3::S3System};

#[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
#[serde(tag = "type")]
pub enum StorageConfig {
    FileSystem { base_path: PathBuf },
    S3 { bucket: String, prefix: String },
}
impl StorageConfig {
    pub fn get_bucket(&self) -> &str {
        match self {
            StorageConfig::FileSystem { base_path } => panic!("No bucket for FileSystem"),
            StorageConfig::S3 { bucket, prefix } => bucket,
        }
    }
        pub fn get_prefix(&self) -> &str {
            match self {
                StorageConfig::FileSystem { base_path } => panic!("No prefix for FileSystem"),
                StorageConfig::S3 { bucket, prefix } => prefix,
            }
    }

    pub fn build_s3(&self) -> S3System {
        todo!()
    }

    pub fn build_fs(&self) -> FileSystem {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::StorageConfig;
    use std::path::PathBuf;

    #[test]
    fn test_something() {
        // let config = StorageConfig::S3 {
        //     bucket: "myBucket".into(),
        //     prefix: "myPrefix".into(),
        // };

        let mut d = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        d.push("resources/test/config_test.toml");
        let config = StorageConfig::build_s3(d);

        assert_eq!("myBucket",config.get_bucket());
        assert_eq!("myPrefix",config.get_prefix());

        //println!("{}", toml::to_string_pretty(&config).unwrap());
    }
}
