mod error;
mod etc;
mod storage;

use error::ABCDResult;
use etc::config::Config;
use rand::distributions::WeightedIndex;
use rand::prelude::*;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::fmt::Debug;
use storage::Storage;
// use anyhow::{Result, Context};

// pub struct Random {}
pub type Random = ThreadRng;

pub trait Model {
    type Parameters: DeserializeOwned + Debug;

    fn prior_sample(&self, random: &Random) -> Self::Parameters; //TODO check density of sampled value is NOT 0
    fn prior_density(&self, p: &Self::Parameters) -> f64;

    fn perturb(&self, p: &Self::Parameters) -> Self::Parameters;
    fn pert_density(&self, a: &Self::Parameters, b: &Self::Parameters) -> f64;

    fn score(&self, p: &Self::Parameters) -> f64;
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct Particle<P> {
    pub parameters: P,
    scores: Vec<f64>,
    weight: f64,
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub struct Population<P> {
    // generation_number: u16,
    tolerance: f64,
    acceptance: f64,
    normalised_particles: Vec<Particle<P>>,
}
// impl Population {
//     pub fn new(...) -> Self {
//         //TODO ensure the weights are normalised
//     }
// }

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub enum Generation<P> {
    Prior,
    Population { pop: Population<P>, gen_number: u16 },
}
impl<P> Generation<P> {
    pub fn generation_number(&self) -> u16 {
        match self {
            Generation::Prior => 0,
            Generation::Population { gen_number, pop: _ } => *gen_number,
        }
    }
}

pub fn run<M: Model, S: Storage>(
    model: M,
    config: Config,
    storage: S,
    random: &mut Random,
) -> ABCDResult<()> {
    // We assume that the storage has already been 'primed' to contain either
    // a) some kind of marker indicating that we're using a prior at gen 0
    //   or
    // b) a population from which we are resuming

    loop {
        let generation_number = storage.check_active_gen()?;
        if generation_number == config.job.num_generations && config.job.terminate_at_target_gen {
            break;
        }
        let gen = storage.retrieve_previous_gen()?;

        loop {
            // Particle loop
            // TODO loop could go on forever?  Use some kind of timeout, or issue warning?
            // (B3) sample a (fitting) parameter set from gen (perturb based on weights and kernel if sampling from generation)
            // (B4) Check if prior probability is zero - if so sample again
            let p = sample_and_perturb_with_support(&gen, &model, random);

            let scores: Option<Vec<f64>> = (0..config.job.num_replicates)
                .map(|rep_idx| {
                    // Reps loop
                    // Check with the filesystem that we are still working on the gen,
                    // else return None, causing the loop to exit.
                    if storage.check_active_gen().ok()? != gen.generation_number() {
                        None
                    } else {
                        // (B5a) run the model once to get a score
                        Some(model.score(&p))
                    }
                })
                .collect();

            match scores {
                Some(scores) => {
                    // We now have a collection of scores for the particle
                    // (B5b) Calculate f^hat by calc'ing proportion less than tolerance
                    // (B6) Calculate not_normalised_weight for each particle from its f^hat (f^hat(p) * prior(p)) / denom)
                    // Save the non_normalised particle to storage
                    // Check if we now have the req'd num particles/reps, if so, break
                    todo!();
                    //weigh_and_save_new_scored_particle(scores);
                    if storage.num_particles_available()? >= config.job.num_particles {
                        // Load all the non_normalised particles
                        // (B7) Normalise all the weights together
                        // Save generation to storage
                        todo!()
                        // flush_entire_generation();
                    }
                }
                None => break, // Is this right, just go round the loop again?
            }
        }
    }

    Ok(())
}

fn sample_and_perturb_with_support<M>(
    gen: &Generation<M::Parameters>,
    model: &M,
    random: &mut Random,
) -> M::Parameters
where
    M: Model,
{
    loop {
        let proposed: M::Parameters = match gen {
            Generation::Prior => model.prior_sample(random),
            Generation::Population {
                gen_number,
                ref pop,
            } => {
                //https://rust-random.github.io/rand/rand/distributions/weighted/struct.WeightedIndex.html
                // 1. sample a particle from the previosu population
                let particle_weights: Vec<f64> =
                    pop.normalised_particles.iter().map(|p| p.weight).collect();

                let dist = WeightedIndex::new(&particle_weights).unwrap();
                let sampled_particle_index = dist.sample(random);
                let sample_particle = &pop.normalised_particles[sampled_particle_index];
                // 2. perturb it with model.perturb(p)
                model.perturb(&sample_particle.parameters)
            }
        };

        if model.prior_density(&proposed) > 0.0 {
            return proposed;
        }
        //TODO warn if loop too many times
    }
}


#[cfg(test)]
pub mod test_helper {
    use std::path::PathBuf;

    pub fn local_test_file_path(string_path: &str) -> PathBuf {
        let mut d = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        d.push(string_path);
        d
    }
}