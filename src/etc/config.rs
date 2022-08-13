use std::error::Error;
use std::fmt::Debug;
use std::path::{Path, PathBuf};

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
    pub tolerance_descent_percentile: usize,
    pub max_num_failures: usize
}
#[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
pub struct RunConfig {
    pub base_config_path: String,
}

#[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
pub struct Config {
    pub storage: StorageConfig,
    pub job: Job,
    pub algorithm: Algorithm,
    pub run: RunConfig
}
impl Config {
    pub fn from_path<P>(config_path: P) -> Result<Self, std::io::Error>
    where
        P: AsRef<Path> + Debug,
    {
        let str = std::fs::read_to_string(config_path.as_ref())?;
        let config: Config = toml::from_str(&str).unwrap();
        log::info!("Loading config: {:#?}", config);
        Ok(config)
    }
}
