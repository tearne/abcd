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

    loop { //Generation loop
        loop { // Particle loop
            // (B3) sample a (fitting) parameter set from it (perturb based on weights and kernel if sampling from posterior)
            // (B4) Check if prior probability is zero - if so sample again
            // (B5a) run the model num_reps times to get an array of scores
            // (B5b) Calculate f^hat by calc'ing proportion less than tolerance
            // (B6) Calculate not_normalised_weight for each particle from its f^hat (f^hat(p) * prior(p)) / denom)
            // Save the non_normalised particle to storage
            // Check if we now have the req'd num particles/reps, if so, break
        }
        // Load all the non_normalised particles
        // (B7) Normalise all the weights together
        // Save generation
    }
}