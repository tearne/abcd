pub mod error;
pub mod etc;
pub mod storage;
pub mod types;
pub mod generation;

use error::{ABCDError, ABCDResult};
use etc::config::Config;
use generation::{PriorGeneration, GenerationOps};
use rand::prelude::*;
use storage::Storage;
pub use types::{ Model, Generation, Particle, Population};

use crate::generation::EmpiricalGeneration;

pub fn run<M: Model, S: Storage>(
    model: M,
    config: Config,
    storage: S,
    random: &mut ThreadRng,
) -> ABCDResult<()> {
//TODO think about how to handle errors at this level (e.g. tolerance being nan)

    match do_gen(
        &storage, 
        &model, 
        &config, 
        random, 
        PriorGeneration{}
    ) {
        Ok(gen_num) if gen_num == 1 => {
            Ok(())
        },
        Ok(gen_num) => Err(ABCDError::AlgortihmError(
            format!("Generation {} was unexpectedly returned from the initial generation.", gen_num)
        )),
        Err(ABCDError::WasWorkingOnAnOldGeneration(msg)) => {
            log::warn!("{}", msg);
            Ok(())
        }
        Err(e) => Err(e),
    }?;

    loop {
        let gen = storage.load_previous_gen()?;

        gen.pop.normalised_particles().iter()
        .for_each(|p| println!("scores after loading previous gen number ({}) {:?}",gen.number, p.scores) );

        log::info!("Loaded generation {}.  Starting the next...", gen.number);
        let completed_gen_number = do_gen(
            &storage,
            &model,
            &config,
            random,
            EmpiricalGeneration::new(gen, config.clone()),
        ); 
        let number = match completed_gen_number {
            Ok(n) => {
                log::info!("... completed generation {}", n);
                Ok(n)
            },
            Err(ABCDError::WasWorkingOnAnOldGeneration(msg)) => {
                log::warn!("Start another generation attempt.  Cause: {}", msg);
                continue
            }
            Err(e) => Err(e),
        }?;
        println!("You are at generation number {number}, target is {}", config.job.num_generations);
        if number == config.job.num_generations && config.job.terminate_at_target_gen {
            break;
        }
    }

    Ok(())
}

fn do_gen<M: Model, S: Storage>(
    storage: &S,
    model: &M,
    config: &Config,
    random: &mut ThreadRng,
    thing_containing_prev_gen: impl GenerationOps<M::Parameters>,
) -> ABCDResult<u16> {
    log::info!("Starting building a new generation from {}", thing_containing_prev_gen.generation_number());

    // let prev_gen_number = storage.previous_gen_number()?;
    //TODO what if this differs from the gen that was passed in?
    let tolerance = thing_containing_prev_gen.calculate_tolerance()?;

    let mut failures = 0;

    loop {
        if failures > config.algorithm.max_num_failures {
            return Err(ABCDError::AlgortihmError("Too many particle failures".into())); //TODO make this different to the particle propose error
        }
        //Particle loop

        // Particle loop
        // (B3) sample a (fitting) parameter set from gen (perturb based on weights and kernel if sampling from generation)
        // (B4) Check if prior probability is zero - if so sample again
        
        let proposed_result = {
            let sampled = thing_containing_prev_gen.sample(model, random);
            if model.prior_density(&sampled) == 0.0 {
                Err(ABCDError::AlgortihmError("Sampled particle out of prior bounds.".into()))
            } else {
                thing_containing_prev_gen.perturb(&sampled, model, random)
            }
        };
        
        let parameters: <M as Model>::Parameters = match proposed_result {
            //TODO does it make sense to put this lot in the propose function?
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
                if storage.previous_gen_number().unwrap() != thing_containing_prev_gen.generation_number() { //TODO cf comment above about what if gen passed in doesn't match
                    Err(ABCDError::WasWorkingOnAnOldGeneration("bad".into()))
                } else {
                    // (B5a) run the model once to get a score
                    Ok(model.score(&parameters)?)
                }
            })
            .collect();

        let scores = scores?; //Throws out to main generation loop, which reloads the previous generation and tries again
        log::info!("Scores are {:?}",scores);

        // We now have a collection of scores for the particle
        // (B5b) Calculate f^hat by calc'ing proportion less than tolerance
        // (B6) Calculate not_normalised_weight for each particle from its f^hat (f^hat(p) * prior(p)) / denom)
        let particle: Particle<M::Parameters> = thing_containing_prev_gen.weigh(parameters, scores, tolerance, model);

        // Save the non_normalised particle to storage
        let save_result = storage.save_particle(&particle); //TODO log if can't save, then try again?  Blow up?  Need to think about this
        match save_result {
            Ok(_save_result) => Ok(()),
            Err(e) => {
                log::error!("Problems saving particle to storage: {}", e);
                Err(ABCDError::StorageInitError)
            }
        }?;

        // Check if we now have the req'd num particles/reps, if so, break
        let num_accepted = storage.num_accepted_particles()?;
        if num_accepted < config.job.num_particles {
            log::info!("There are {num_accepted} accepted particles in the bucket.");
        } else {
            // Load all the non_normalised particles
            let particles: Vec<Particle<M::Parameters>> = storage.load_accepted_particles()?; //TODO think about the error case, tries again?
            let rejections = storage.num_rejected_particles()?; //TODO think about the error case, tries again?
            let acceptance = {
                let num: f64 = cast::f64(particles.len()); //TODO check we understand this, seems to be infallable??!
                let rejected: f64 =  cast::f64(rejections);
                (num / (num + rejected)) as f32
            };
            log::info!("Acceptance rate was {acceptance:.3}");
            log::info!("Tolerance is {tolerance:.3}");

            let new_generation = Generation::new(particles, thing_containing_prev_gen.generation_number() + 1, tolerance, acceptance);

            new_generation.pop.normalised_particles().iter()
            .for_each(|p| println!("scores after new gen {:?}",p.scores) );

            // Save the non_normalised particle to storage
            let save_gen_result = storage.save_new_gen(&new_generation); //TODO log if can't save, then try again?  Blow up?  Need to think about this

            match save_gen_result {
                Ok(_) => Ok(()), 
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
