mod storage;
mod etc;
mod error;

use error::ABCDResult;
use etc::config::Config;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use storage::Storage;
use std::fmt::Debug;
// use anyhow::{Result, Context};

pub trait Random {}

pub trait Model {
    type Parameters: DeserializeOwned + Debug;

    fn prior_sample<R: Random>(&self, random: &R) -> Self::Parameters; //TODO check density of sampled value is NOT 0
    fn prior_density(&self, p: Self::Parameters) -> f64;

    fn perturb(&self, p: Self::Parameters) -> Self::Parameters;
    fn pert_density(&self, a: Self::Parameters, b: Self::Parameters) -> f64;

    fn score(&self, p: Self::Parameters) -> f64;
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct Particle<P> {
    parameters: P,
    scores: Vec<f64>,
    weight: f64,
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub struct Population<P> {
    generation_number: u16,
    tolerance: f64,
    acceptance: f64,
    particles: Vec<Particle<P>>,
}

pub enum Generation<P> {
    Prior,
    Population(Population<P>)
}

pub fn run<M: Model, S: Storage>(model: M, config: Config, storage: S) -> ABCDResult<()>{

    for gen_id in 0..config.job.num_generations { //Generation loop
        let mut gen: Generation::<M::Parameters> = 
            if gen_id == 0 {
                Generation::Prior
            } else {
                Generation::Population(
                    storage.retrieve_previous_gen()?
                )
            };


        loop { // Particle loop
            // (B3) sample a (fitting) parameter set from it (perturb based on weights and kernel if sampling from posterior)
            // (B4) Check if prior probability is zero - if so sample again
            loop { // Reps loop
                // (B5a) run the model once to get a score
                // Check with the filesystem that we are still working on the gen, else abort out to gen loop
            }
            // We now have an array of score for the particle
            // (B5b) Calculate f^hat by calc'ing proportion less than tolerance
            // (B6) Calculate not_normalised_weight for each particle from its f^hat (f^hat(p) * prior(p)) / denom)
            // Save the non_normalised particle to storage
            // Check if we now have the req'd num particles/reps, if so, break
        }
        // Load all the non_normalised particles
        // (B7) Normalise all the weights together
        // Save generation
    }

    Ok(())
}