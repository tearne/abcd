mod error;
mod etc;
mod storage;
mod types;

use error::{ABCDError, ABCDResult};
use etc::config::Config;
use rand::prelude::*;
use serde::Deserializer;
use storage::Storage;
pub use types::{ Model, Generation, Particle};
use statrs::statistics::{Statistics,OrderStatistics, Data};
use rand::distributions::WeightedIndex;

pub fn run<M: Model, S: Storage>(
    model: M,
    config: Config,
    storage: S,
    random: &mut ThreadRng,
) -> ABCDResult<()> {
//TODO think about how to handle errors at this level (e.g. tolerance being nan)

    match do_gen(&storage, &model, &config, random, PriorGeneration{}) {
        Ok(gen_number) if gen_number == 1 => Ok(()),
        Ok(gen_number) => Err(ABCDError::AlgortihmError("Should have generation 1 in prior generation".into())),
        Err(ABCDError::WasWorkingOnAnOldGeneration(msg)) => {
            log::warn!("{}", msg);
            Ok(())
        }
        Err(e) => Err(e),
    }?;

    loop {
        let gen = storage.load_previous_gen()?;
        let number = do_gen(
            &storage,
            &model,
            &config,
            random,
            EmpiricalGeneration{gen, config: config.clone()},
        ); 
        let number = match number {
            Ok(number) => Ok(number),
            Err(ABCDError::WasWorkingOnAnOldGeneration(msg)) => {
                log::warn!("{}", msg);
                continue
            }
            Err(e) => Err(e),
        }?;
        if number == config.job.num_generations && config.job.terminate_at_target_gen {
            break;
        }
    }

    Ok(())
}

trait GenerationOps<P> {
    //TODO so we can reuse these ops in the diagnostic runner
    // fn sample<M: Model<Parameters = P>>(&self, model: &M, random: &mut ThreadRng) -> ABCDResult<P>;
    // fn sample_and_perturb<M: Model<Parameters = P>>(&self, model: &M, random: &mut ThreadRng) -> ABCDResult<P>;
    fn propose<M: Model<Parameters = P>>(&self, model: &M, random: &mut ThreadRng) -> ABCDResult<P>;
    
    
    fn calculate_tolerance(&self) -> ABCDResult<f64>;
    fn weigh<M: Model<Parameters = P>>(&self, params: P, scores: Vec<f64>, tolerance: f64, model: &M) -> Particle<P>;

    fn calculate_fhat(scores: &Vec<f64>, tolerance: f64) -> f64 {
        // (B5b) Calculate f^hat by calc'ing proportion less than tolerance
        let number_reps = cast::f64(scores.len());
        let number_reps_less_than_tolerance = scores
            .iter()
            .filter(|score| **score < tolerance)
            .count();
        cast::f64(number_reps_less_than_tolerance)/number_reps
    }
}
struct EmpiricalGeneration<P>{
    gen: Generation<P>,
    config: Config
}
impl<P> GenerationOps<P> for EmpiricalGeneration<P> {
    fn propose<M: Model<Parameters = P>>(&self, model: &M, random: &mut ThreadRng) -> ABCDResult<P> {
        //https://rust-random.github.io/rand/rand/distributions/weighted/struct.WeightedIndex.html
        // 1. sample a particle from the previosu population
        // TODO can't we pre-calculate the weights table to avoid rebuilding on every proposal?
        let particle_weights: Vec<f64> = self.gen
            .pop
            .normalised_particles()
            .iter()
            .map(|p| p.weight)
            .collect();

            let dist = WeightedIndex::new(&particle_weights).unwrap();
            let sampled_particle_index: usize = dist.sample(random);
            let sample_particle = &self.gen.pop.normalised_particles()[sampled_particle_index];
        // 2. perturb it with model.perturb(p)
        let params = model.perturb(&sample_particle.parameters,random);        
        if model.prior_density(&params) > 0.0 {
            Ok(params)
        } else {
            Err(ABCDError::AlgortihmError("Proposed particle out of prior bounds.".into()))
        }
    }

    fn calculate_tolerance(&self) -> ABCDResult<f64> {
        // Get distribution of scores from last generation then reduce by tolerance descent rate (configured) - crate exists for percentile => 
        let score_distribution: ABCDResult<Vec<f64>> = self.gen
            .pop
            .normalised_particles()
            .iter()
            .map(|particle| {
                let mean_scores: f64 = particle.scores.clone().mean();
                assert!(!mean_scores.is_nan()); //TODO Put proper ABCDError here
                match mean_scores.is_nan() {
                    true => Ok(mean_scores),
                    false => Err(ABCDError::AlgortihmError("Mean score is not a number.".into()))
                }
            })
            .collect();

        let mut score_distribution = Data::new(score_distribution?);
        let new_tolerance = score_distribution.percentile(self.config.algorithm.tolerance_descent_percentile);

        match new_tolerance.is_nan() {
            true => Ok(new_tolerance),
            false => Err(ABCDError::AlgortihmError("Tolerance is not a number.".into()))
        }
    }

    fn weigh<M: Model<Parameters = P>>(&self, parameters: P, scores: Vec<f64>, tolerance: f64, model: &M) -> Particle<P> {
        // (B6) Calculate not_normalised_weight for each particle from its f^hat (f^hat(p) * prior(p)) / denom)
        let fhat = Self::calculate_fhat(&scores, tolerance);
        let prior_prob = model.prior_density(&parameters);
        let denominator : f64 = self.gen.pop.normalised_particles()
                .iter()
                .map(|prev_gen_particle| {
                    let weight = prev_gen_particle.weight;
                    let pert_density = model.pert_density(&prev_gen_particle.parameters, &parameters);
                    weight * pert_density
                }).sum();
        let weight = fhat*prior_prob / denominator;
        Particle { 
            parameters, 
            scores, 
            weight 
        }
    }
}
struct PriorGeneration{}
impl<P> GenerationOps<P> for PriorGeneration {
    fn propose<M: Model<Parameters = P>> (&self, model: &M, random: &mut ThreadRng) -> ABCDResult<P> {
        Ok(model.prior_sample(random))
    }

    fn calculate_tolerance(&self) -> ABCDResult<f64> {
        Ok(f64::MAX)
    }


    fn weigh<M: Model<Parameters = P>>(&self, parameters: P, scores: Vec<f64>, tolerance: f64, _model: &M) -> Particle<P> {
        let fhat = <Self as GenerationOps<P>>::calculate_fhat(&scores, tolerance);
        Particle { 
            parameters, 
            scores, 
            weight:fhat 
        }
    }

}

fn do_gen<M: Model, S: Storage>(
    storage: &S,
    model: &M,
    config: &Config,
    random: &mut ThreadRng,
    gen_stuff: impl GenerationOps<M::Parameters>,
) -> ABCDResult<u16> {
    let prev_gen_number = storage.previous_gen_number()?;
    let tolerance = gen_stuff.calculate_tolerance()?;

    let mut failures = 0;

    loop {
        if failures > config.algorithm.max_num_failures {
            return Err(ABCDError::AlgortihmError("Too many particle failures".into())); //TODO make this different to the particle propose error
        }
        //Particle loop

        // Particle loop
        // (B3) sample a (fitting) parameter set from gen (perturb based on weights and kernel if sampling from generation)
        // (B4) Check if prior probability is zero - if so sample again
        let parameters: <M as Model>::Parameters = match gen_stuff.propose(model, random){
            //TODO put this lot in the propose function?
            Ok(parameters) => Ok(parameters),
            Err(ABCDError::AlgortihmError(msg)) => {
                log::warn!("{}", msg);
                failures += 1;
                continue;
            },
            Err(e) => Err(e),
        }?;

        let scores: ABCDResult<Vec<f64>> = (0..config.job.num_replicates)
            .map(|_| {
                if storage.previous_gen_number().unwrap() != prev_gen_number {
                    Err(ABCDError::WasWorkingOnAnOldGeneration("bad".into()))
                } else {
                    // (B5a) run the model once to get a score
                    Ok(model.score(&parameters))
                }
            })
            .collect();

        let scores = scores?; //Throws out to main generation loop, which reloads the previous generation and tries again

        // We now have a collection of scores for the particle
        // (B5b) Calculate f^hat by calc'ing proportion less than tolerance
        // (B6) Calculate not_normalised_weight for each particle from its f^hat (f^hat(p) * prior(p)) / denom)
        let particle: Particle<M::Parameters> = gen_stuff.weigh(parameters, scores, tolerance, model);


        // Save the non_normalised particle to storage
        let save_result = storage.save_particle(&particle); //TODO log if can't save, then try again?  Blow up?  Need to think about this
        let save_result = match save_result {
            Ok(save_result) => Ok(save_result), //This just holds path of where particle was saved - do we want to log that?
            Err(e) => {
                log::error!("Problems saving particle to storage: {}", e);
                Err(ABCDError::StorageInitError)
            }
        }?;

        // Check if we now have the req'd num particles/reps, if so, break
        if storage.num_accepted_particles()? >= config.job.num_particles {
            // Load all the non_normalised particles
            let particles: Vec<Particle<M::Parameters>> = storage.load_accepted_particles()?; //TODO think about the error case, tries again?
            // let particles = storage.load_accepted_particles(); 
            // let particles = match particles {
            //     Ok(particles) => Ok(particles), 
            //     Err(ABCDError::Other(msg)) => {
            //         log::error!("{}", msg);
            //         Err(ABCDError::Other(msg))          }
            //     Err(ABCDError::SerdeError(msg)) => {
            //             log::error!("{}", msg);
            //             Err(ABCDError::SerdeError(msg) )          }
            //     Err(e) => Err(e),
            // }?;
            let rejections = storage.num_rejected_particles()?; //TODO think about the error case, tries again?
            // let rejections = storage.num_rejected_particles();
            // let rejections = match rejections {
            //     Ok(rejections) => Ok(rejections), 
            //     Err(ABCDError::StorageConsistencyError(msg)) => {
            //         log::error!("{}", msg);
            //         Err(ABCDError::StorageConsistencyError(msg))          }
            //     Err(e) => Err(e),
            // }?;
            let acceptance = {
                let num: f64 = cast::f64(particles.len()); //TODO check we understand this, seems to be infallable??!
                let rejected: f64 =  cast::f64(rejections);
                num / (num + rejected)
            };
             let new_generation = Generation::new( particles, prev_gen_number + 1, tolerance, acceptance);
            // Save generation to storage
            // storage.save_new_gen(&new_generation)?; //TODO think about the error case, tries again?
            // Save the non_normalised particle to storage
            let save_gen_result = storage.save_new_gen(&new_generation); //TODO log if can't save, then try again?  Blow up?  Need to think about this
            let save_gen_result = match save_gen_result {
                Ok(save_gen_result) => Ok(save_gen_result), 
                Err(ABCDError::StorageConsistencyError(msg)) => {
                       log::error!("{}", msg);
                       Err(ABCDError::StorageConsistencyError(msg) )          }   
                Err(ABCDError::GenAlreadySaved(msg)) => {
                        log::error!("{}", msg);
                        Err(ABCDError::GenAlreadySaved(msg) )          }             
                Err(e) => Err(e),
            }?;
             return Ok(new_generation.number);
        }
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
