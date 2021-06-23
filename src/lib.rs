mod storage;
mod error;

use serde::{Serialize, Deserialize};
use std::cmp::Ordering;
use std::path::{Path, PathBuf};
use std::fmt::Debug;

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
pub struct s3_params {
    input_data_root: PathBuf,   //TODO can we make this a Path? lifetime seems to clash
}
impl s3_params {
    pub fn absoluteify_root_path(&mut self, config_path: impl AsRef<Path>) {
        if !self.input_data_root.starts_with("/") {
            self.input_data_root = config_path
                .as_ref()
                .parent()
                .unwrap()
                .join(self.input_data_root.as_path())
        };
    }
}


#[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
pub struct Config {
    pub s3_params: s3_params
}
impl Config {
    pub fn from_path<P>(config_path: P) -> Self
    where
        P: AsRef<Path> + Debug,
    {
        let str = std::fs::read_to_string(config_path.as_ref())
            .unwrap_or_else(|e| panic!("Failed to load config from {:?}: {}", config_path, e));
        let mut config: Self = toml::from_str(&str).unwrap();
        config.s3_params.absoluteify_root_path(config_path);
        config
    }
}

