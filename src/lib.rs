mod algorithm;
mod error;
mod etc;
mod storage;
mod types;

use error::{ABCDError, ABCDResult};
use etc::config::Config;
use rand::prelude::*;
use storage::Storage;
pub use types::{ Model, Generation, Particle};
use statrs::statistics::{Statistics,OrderStatistics, Data};
use rand::distributions::WeightedIndex;

pub type Random = ThreadRng;

pub fn run<M: Model, S: Storage>(
    model: M,
    config: Config,
    storage: S,
    random: &mut Random,
) -> ABCDResult<()> {
    match do_gen(&storage, &model, &config, random, PriorGeneration{}) {
        Ok(gen_number) if gen_number == 1 => (),
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
            EmpiricalGeneration{gen, config: config.clone()},
        )?;
        if number == config.job.num_generations && config.job.terminate_at_target_gen {
            break;
        }
    }

    Ok(())
}

trait GenerationOps<P> {
    fn propose<M: Model<Parameters = P>>(&self, model: &M, random: &ThreadRng) -> P;
    fn calculate_tolerance(&self) -> f64;
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
    fn propose<M: Model<Parameters = P>>(&self, model: &M, random: &ThreadRng) -> P {
        //todo!()
        let proposed: M::Parameters = {
            // //https://rust-random.github.io/rand/rand/distributions/weighted/struct.WeightedIndex.html
            // // 1. sample a particle from the previosu population
            // let particle_weights: Vec<f64> = self.gen
            //     .pop
            //     .normalised_particles
            //     .iter()
            //     .map(|p| p.weight)
            //     .collect();

            //  let dist = WeightedIndex::new(&particle_weights).unwrap();
            //  let sampled_particle_index = dist.sample(random);
            //  let sample_particle = self.gen.pop.normalised_particles[sampled_particle_index];
            // // 2. perturb it with model.perturb(p)
            // model.perturb(&sample_particle.parameters);
            todo!()
        };

        // if model.prior_density(&proposed) > 0.0 {
        //     return proposed;
        // }
        //TODO warn if loop too many times
    }

    fn calculate_tolerance(&self) -> f64 {
        // Get distribution of scores from last generation then reduce by tolerance descent rate (configured) - crate exists for percentile => 
        let score_distribution: Vec<f64> = self.gen
            .pop
            .normalised_particles
            .iter()
            .map(|particle| {
                let mean_scores: f64 = particle.scores.clone().mean();
                assert!(!mean_scores.is_nan()); //TODO Put proper ABCDError here
                mean_scores
            })
            .collect();

        let mut score_distribution = Data::new(score_distribution);
        let new_tolerance = score_distribution.percentile(self.config.algorithm.tolerance_descent_percentile);
        assert!(!new_tolerance.is_nan()); //TODO Put proper ABCDError here
        new_tolerance
    }

    fn weigh<M: Model<Parameters = P>>(&self, parameters: P, scores: Vec<f64>, tolerance: f64, model: &M) -> Particle<P> {
        // (B6) Calculate not_normalised_weight for each particle from its f^hat (f^hat(p) * prior(p)) / denom)
        let fhat = Self::calculate_fhat(&scores, tolerance);
        let prior_prob = model.prior_density(&parameters);
        let denominator : f64 = self.gen.pop.normalised_particles
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
    fn propose<M: Model<Parameters = P>> (&self, model: &M, random: &ThreadRng) -> P {
        model.prior_sample(random)
    }

    fn calculate_tolerance(&self) -> f64 {
        f64::MAX
    }


    fn weigh<M: Model<Parameters = P>>(&self, parameters: P, scores: Vec<f64>, tolerance: f64, model: &M) -> Particle<P> {
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
    random: &mut Random,
    gen_stuff: impl GenerationOps<M::Parameters>,
) -> ABCDResult<u16> {
    let prev_gen_number = storage.previous_gen_number()?;
    let tolerance = gen_stuff.calculate_tolerance();
    loop {
        //Particle loop

        // Particle loop
        // TODO loop could go on forever?  Use some kind of timeout, or issue warning?
        // (B3) sample a (fitting) parameter set from gen (perturb based on weights and kernel if sampling from generation)
        // (B4) Check if prior probability is zero - if so sample again
        let parameters: <M as Model>::Parameters = gen_stuff.propose(model, random);

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
        let particle: Particle<M::Parameters> = gen_stuff.weigh(parameters, scores, tolerance, model);

        // Save the non_normalised particle to storage
        storage.save_particle(&particle)?; 

        // Check if we now have the req'd num particles/reps, if so, break
        if storage.num_accepted_particles()? >= config.job.num_particles {
            // Load all the non_normalised particles
            let particles: Vec<Particle<M::Parameters>> = storage.load_accepted_particles()?;
            let rejections = storage.num_rejected_particles()?;
            let acceptance = {
                let num: f64 = cast::f64(particles.len()); //TODO check we understand this, seems to be infallable??!
                let rejected: f64 =  cast::f64(rejections);
                num / (num + rejected)
            };
             let new_generation = Generation::new( particles, prev_gen_number + 1, tolerance, acceptance);
            // Save generation to storage
             storage.save_new_gen(&new_generation);
             return Ok(new_generation.number);
        }
    }
}


//TODO put this in the previous gen proposer
fn sample_and_perturb_with_support<M> (
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
            // let particle_weights: Vec<f64> = gen
            //     .pop
            //     .normalised_particles
            //     .iter()
            //     .map(|p| p.weight)
            //     .collect();

            // let dist = WeightedIndex::new(&particle_weights).unwrap();
            // let sampled_particle_index = dist.sample(random);
            //let sample_particle = &gen.pop.normalised_particles[sampled_particle_index];
            // 2. perturb it with model.perturb(p)
            //model.perturb(&sample_particle.parameters)
            todo!()
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
