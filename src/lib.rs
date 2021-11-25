mod error;
mod storage;
mod etc;

use etc::config::Config;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;

pub trait Random {}

pub trait Model {
    type Parameters;

    fn prior_sample<R: Random>(&self, random: &R) -> Self::Parameters; //TODO check density of sampled value is NOT 0
    fn prior_density(&self, p: Self::Parameters) -> f64;

    fn perturb(&self, p: Self::Parameters) -> Self::Parameters;
    fn pert_density(&self, a: Self::Parameters, b: Self::Parameters) -> f64;

    fn score(&self, p: Self::Parameters) -> f64;
}

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

pub fn run<M: Model>(m: M, config: Config) {
    // load the prior/generation

    loop {
        // sample a (fitting) parameter set from it
        // run the model num_reps times to get an array of scores
        // Weigh the scores to get a particle
        // Save the particle to storage
    }
}