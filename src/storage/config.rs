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

    #[test]
    fn test_something() {
        let config = StorageConfig::S3 {
            bucket: "myBucket".into(),
            prefix: "myPrefix".into(),
        };

        println!("{}", toml::to_string_pretty(&config).unwrap());
    }
}
