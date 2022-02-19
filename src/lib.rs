mod algorithm;
mod error;
mod etc;
mod storage;

use error::{ABCDError, ABCDResult};
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
    type Parameters: Serialize + DeserializeOwned + Debug;

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
pub struct Generation<P> {
    pop: Population<P>,
    number: u16,
}

pub fn run<M: Model, S: Storage>(
    model: M,
    config: Config,
    storage: S,
    random: &mut Random,
) -> ABCDResult<()> {
    match do_gen(&storage, &model, &config, random, Prior::new()) {
        Ok(gen_number) if gen_number == 0 => (),
        Err(ABCDError::WasWorkingOnAnOldGeneration(_)) => {
            println!("LOG ME");
            ()
        }
        Err(e) => Err(e)?,
        _ => unreachable!(),
    };

    loop {
        let gen = storage.load_previous_gen()?;
        let number = do_gen(
            &storage,
            &model,
            &config,
            random,
            PreviousGeneration::new(gen),
        )?;
        if number == config.job.num_generations && config.job.terminate_at_target_gen {
            break;
        }
    }

    todo!()
}

trait Proposer<M: Model> {
    fn next(&self, model: &M, random: &mut Random) -> M::Parameters;
}
struct Prior {}
impl Prior {
    pub fn new() -> Self {
        Prior {}
    }
}
impl<M: Model> Proposer<M> for Prior {
    fn next(&self, model: &M, random: &mut Random) -> <M as Model>::Parameters {
        model.prior_sample(random)
    }
}

struct PreviousGeneration<P> {
    generation: Generation<P>,
}
impl<P> PreviousGeneration<P> {
    pub fn new(generation: Generation<P>) -> Self {
        PreviousGeneration { generation }
    }
}
impl<M: Model> Proposer<M> for PreviousGeneration<M::Parameters> {
    fn next(&self, model: &M, random: &mut Random) -> <M as Model>::Parameters {
        sample_and_perturb_with_support(&self.generation, model, random)
    }
}

fn do_gen<M: Model, S: Storage>(
    storage: &S,
    model: &M,
    config: &Config,
    random: &mut Random,
    proposer: impl Proposer<M>,
) -> ABCDResult<u16> {
    let prev_gen_number = storage.previous_gen_number()?;
    loop {
        //Particle loop

        // Particle loop
        // TODO loop could go on forever?  Use some kind of timeout, or issue warning?
        // (B3) sample a (fitting) parameter set from gen (perturb based on weights and kernel if sampling from generation)
        // (B4) Check if prior probability is zero - if so sample again
        let parameters: <M as Model>::Parameters = proposer.next(model, random);

        let scores: ABCDResult<Vec<f64>> = (0..config.job.num_replicates)
            .map(|rep_idx| {
                if storage.previous_gen_number().unwrap() != prev_gen_number {
                    Err(ABCDError::WasWorkingOnAnOldGeneration("bad".into()))
                } else {
                    // (B5a) run the model once to get a score
                    Ok(model.score(&parameters))
                }
            })
            .collect();

        let scores = scores?;

        // We now have a collection of scores for the particle
        // (B5b) Calculate f^hat by calc'ing proportion less than tolerance
        // (B6) Calculate not_normalised_weight for each particle from its f^hat (f^hat(p) * prior(p)) / denom)
        let particle = algorithm::weigh_particle(scores, f64::MAX, model);
        // let particle = Particle{
        //     parameters,
        //     scores,
        //     weight,
        // };

        // Save the non_normalised particle to storage
        storage.save_particle(&particle)?;

        // Check if we now have the req'd num particles/reps, if so, break
        if storage.num_working_particles()? >= config.job.num_particles {
            // Load all the non_normalised particles
            let particles: Vec<Particle<M::Parameters>> = storage.load_working_particles()?;

            // (B7) Normalise all the weights together
            let new_generation = algorithm::normalise::<M>(particles, prev_gen_number + 1);

            // Save generation to storage
            storage.save_new_gen(&new_generation);

            return Ok(new_generation.number);
        }
    }
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
        let proposed: M::Parameters = {
            //  gen match {
            //Generation::Prior => model.prior_sample(random),
            // Generation::Population {
            //     gen_number,
            //     ref pop,
            // } => {
            //https://rust-random.github.io/rand/rand/distributions/weighted/struct.WeightedIndex.html
            // 1. sample a particle from the previosu population
            let particle_weights: Vec<f64> = gen
                .pop
                .normalised_particles
                .iter()
                .map(|p| p.weight)
                .collect();

            let dist = WeightedIndex::new(&particle_weights).unwrap();
            let sampled_particle_index = dist.sample(random);
            let sample_particle = &gen.pop.normalised_particles[sampled_particle_index];
            // 2. perturb it with model.perturb(p)
            model.perturb(&sample_particle.parameters)
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

    pub fn test_data_path(proj_path: &str) -> PathBuf {
        let mut d = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        d.push(proj_path);
        d
    }
}
