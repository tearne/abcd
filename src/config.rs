use crate::storage::config::StorageConfig;
use std::fmt::Debug;
use std::path::Path;

#[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
pub struct Job {
    pub num_generations: u16,
    pub terminate_at_target_gen: bool,
    pub num_particles: u32,
}

#[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
pub struct Algorithm {
    pub tolerance_descent_percentile: usize,
    pub max_num_failures: usize,
}

#[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
pub struct Config {
    pub storage: StorageConfig,
    pub job: Job,
    pub algorithm: Algorithm,
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
