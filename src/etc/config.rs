use std::fmt::Debug;
use std::path::Path;

use crate::storage::config::StorageConfig;

// #[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
// pub struct Storage {
//     pub location: String,
//     pub kind: String,
// }
// impl Storage {
//     pub fn get_path_string(&self) -> String {
//         match self.kind.as_str() {
//             "s3" => format!("s3://{}", self.location),
//             "envvar" => {
//                 println!(" --> {}", &self.location);
//                 envmnt::get_or_panic(&self.location)
//             },
//             _ => unimplemented!(),
//         }
//     }
// }

#[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
pub struct Job {
    pub num_generations: u16,
    pub terminate_at_target_gen: bool,
    pub num_replicates: u16,
    pub num_particles: u32,
}

#[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
pub struct Algorithm {
    tolerance_descent_percentile: f32,
}
//TODO validate - https://crates.io/crates/validator

#[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
pub struct Config {
    pub storage: StorageConfig,
    pub job: Job,
    pub algorithm: Algorithm,
}
impl Config {
    pub fn from_path<P>(config_path: P) -> Self
    where
        P: AsRef<Path> + Debug,
    {
        let str = std::fs::read_to_string(config_path.as_ref())
            .unwrap_or_else(|e| panic!("Failed to load config from {:?}: {}", config_path, e));
        log::info!("Loading str: {:#?}", str);
        let config: Config = toml::from_str(&str).unwrap();
        log::info!("Loading config: {:#?}", config);
        config
    }
}

#[cfg(test)]
mod tests {
    use super::Config;
    use crate::test_helper::local_test_file_path;

    #[test]
    fn load_with_env_var_override() {
        envmnt::set("ABCDBucket", "s3://my-env-var-bucket");

        let path = local_test_file_path("resources/test/config_test.toml");
        let config = Config::from_path(path);

        let bucket = match config.storage {
            crate::storage::config::StorageConfig::FileSystem { base_path:_ } => panic!("expected S3 config"),
            crate::storage::config::StorageConfig::S3 { bucket, prefix:_ } => bucket,
        };

        assert_eq!("s3://my-env-var-bucket", bucket);
    }
}
