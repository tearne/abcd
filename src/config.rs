use crate::storage::config::StorageConfig;
use std::borrow::Cow;
use std::fmt::Debug;
use std::path::Path;

#[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
pub struct AbcdConfig {
    pub num_generations: u16,
    pub terminate_at_target_gen: bool,
    pub num_particles: u32,
    pub tolerance_descent_percentile: usize,
    pub max_num_failures: usize,
}

// #[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
// pub struct AbcdConfig {
//     pub storage: StorageConfig,
//     pub job: AbcdJob,
// }
// impl AbcdConfig {
//     pub fn new<S: Into<Cow<'static, str>>>(bucket: S, prefix: S, job: AbcdJob) -> Result<Self, std::io::Error> {
//         Ok(Self {
//             storage: StorageConfig{
//                 bucket: bucket.into(),
//                 prefix: prefix.into(),
//             },
//             job,
//         })
//     }

//     pub fn from_path<P>(config_path: P) -> Result<Self, std::io::Error>
//     where
//         P: AsRef<Path> + Debug,
//     {
//         let str = std::fs::read_to_string(config_path.as_ref())?;
//         let config: AbcdConfig = toml::from_str(&str).unwrap();
//         log::info!("Loading config: {:#?}", config);
//         Ok(config)
//     }
// }
