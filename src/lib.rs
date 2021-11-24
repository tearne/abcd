mod error;
mod storage;
mod etc;

use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::fmt::Debug;
use std::path::{Path, PathBuf};

pub trait Random {}

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
pub struct S3Params {
    input_data_root: PathBuf, //TODO can we make this a Path? lifetime seems to clash
}
impl S3Params {
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
