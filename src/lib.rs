mod storage;
mod etc;
mod error;

use error::ABCDResult;
use etc::config::Config;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use storage::Storage;
use std::fmt::Debug;
// use anyhow::{Result, Context};

pub struct Random {}

pub trait Model {
    type Parameters: DeserializeOwned + Debug;

    fn prior_sample(&self, random: &Random) -> Self::Parameters; //TODO check density of sampled value is NOT 0
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

pub fn run<M: Model, S: Storage>(model: M, config: Config, storage: S, random: &Random) -> ABCDResult<()>{

    let gen: Generation::<M::Parameters> = Generation::Prior;

    for gen_id in 1..=config.job.num_generations { //Generation loop
        loop { // Particle loop
            // TODO loop could go on forever?  Use some kind of timeout, or issue warning?
            // (B3) sample a (fitting) parameter set from gen (perturb based on weights and kernel if sampling from generation)
            // (B4) Check if prior probability is zero - if so sample again
            let p = sample_with_support(gen, model, random);
            
            let scores: Option<Vec<f64>> = (0..config.job.num_replicates).map(|_|{ // Reps loop
                // Check with the filesystem that we are still working on the gen, else abort out to gen loop
                // (B5a) run the model once to get a score
                todo!();
            }).collect();

            match scores {
                Some(scores) => {
                    // We now have a collection of scores for the particle
                    // (B5b) Calculate f^hat by calc'ing proportion less than tolerance
                    // (B6) Calculate not_normalised_weight for each particle from its f^hat (f^hat(p) * prior(p)) / denom)
                    // Save the non_normalised particle to storage
                    // Check if we now have the req'd num particles/reps, if so, break
                    gen = weigh_and_save_new_scored_particle(scores);
                    if gen.has_enough_particles_for_flush() { break; }
                },
                None => break, // Is this right, just go round the loop again?
            }
        }
        // Load all the non_normalised particles
        // (B7) Normalise all the weights together
        // Save generation
        gen = flush();
    }

    Ok(())
}

fn sample_with_support<M>(
    gen: Generation::<M::Parameters>, 
    model: M, 
    random: &Random) -> M::Parameters 
where M: Model {
    loop {
        let proposed: M::Parameters = match gen {
            Generation::Prior => model.prior_sample(random),
            Generation::Population(pop) => {
                //https://rust-random.github.io/rand/rand/distributions/weighted/struct.WeightedIndex.html
                // pop.particles
                todo!()
            },
        };

        if model.prior_density(proposed) > 0.0 {
            return proposed;
        }
        //TODO warn if loop too many times
    }
}