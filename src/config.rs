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
