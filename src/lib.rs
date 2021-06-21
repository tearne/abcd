mod storage;
mod error;

use serde::{Serialize, Deserialize};
use std::cmp::Ordering;

pub trait Random { }

pub trait Model {
    type Parameters;

    fn prior_sample<R: Random>(&self, random: &R) -> Self::Parameters; //TODO check density of sampled value is NOT 0
    fn prior_density(&self, p: Self::Parameters) -> f64;

    fn perturb(&self, p: Self::Parameters) -> Self::Parameters;
    fn pert_density(&self, a: Self::Parameters, b: Self::Parameters) -> f64;

    fn score(&self, p: Self::Parameters) -> f64;
}

// #[derive(Serialize, Deserialize, Debug, PartialEq)]
// struct Scored<P> {
//     parameters: P,
//     score: f64,
// }
// impl<P> Scored<P> {
//     pub fn new(parameters: P, score: f64) -> Scored<P> {
//         Scored{parameters, score}
//     }
// }

#[derive(Serialize, Deserialize, Debug, PartialEq)]
struct Particle<P> {
    parameters: P,
    scores: Vec<f64>,
    weight: f64,
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
struct Generation<P> {
    generation_number: u16,
    tolerance: f64,
    acceptance: f64,
    particles: Vec<Particle<P>>,
}


#[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
pub struct SystemParams {
    input_data_root: PathBuf,   //TODO can we make this a Path? lifetime seems to clash
}
impl SystemParams {
}


#[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
pub struct Config {
    pub system_params: SystemParams
}
impl Config {
    pub fn from_path<P>(config_path: P) -> Self
    where
        P: AsRef<Path> + Debug,
    {
        let str = std::fs::read_to_string(config_path.as_ref())
            .unwrap_or_else(|e| panic!("Failed to load config from {:?}: {}", config_path, e));
        let mut config: Self = toml::from_str(&str).unwrap();
        config.system_params.absoluteify_root_path(config_path);
        config
    }
}

