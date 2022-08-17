pub mod config;
pub mod error;
pub mod storage;
pub mod types;
pub mod wrapper;

use config::Config;
use error::{ABCDErr, ABCDResult};
use rand::prelude::*;
use storage::Storage;
pub use types::{Generation, Model, Particle, Population};
use wrapper::GenWrapper;
pub struct ABCD<M: Model, S: Storage> {
    model: M,
    config: Config,
    storage: S,
}

impl<M: Model, S: Storage> ABCD<M, S> {
    pub fn run(model: M, config: Config, storage: S, rng: &mut impl Rng) -> ABCDResult<()> {
        let abcd = ABCD {
            model,
            config,
            storage,
        };

        match abcd.generation_loop(rng) {
            Err(ABCDErr::StaleGenerationErr(msg)) | Err(ABCDErr::ParticleErr(msg)) => Err(
                ABCDErr::SystemError(format!("Unexpected error cascaded to top of ABCD: {}", msg)),
            ),
            other => other,
        }
    }

    fn generation_loop(&self, rng: &mut impl Rng) -> ABCDResult<()> {
        let mut gen_failures = Vec::<String>::new();

        loop {
            // Generation loop
            if gen_failures.len() > self.config.algorithm.max_num_failures {
                return Err(ABCDErr::TooManyRetriesError(
                    "Too many retries in generation loop".into(),
                    gen_failures,
                ));
            }
            
            let prev_gen = GenWrapper::<M::Parameters>::load_previous_gen::<M, S>(
                &self.storage,
                &self.config,
            )?;

            let new_gen = match self.make_particles_loop(&prev_gen, rng) {
                o @ Ok(_) => o,
                Err(e) => {
                    let msg = format!("In generation loop, failed to make a new generation but will try again: {}", e);
                    log::error!("{}", msg);
                    gen_failures.push(msg);
                    continue;
                }
            }?;

            if new_gen.number == self.config.job.num_generations
                && self.config.job.terminate_at_target_gen
            {
                log::info!("Reached target number of generations.");
                break;
            }
        }

        Ok(())
    }

    fn make_particles_loop(
        &self,
        prev_gen: &GenWrapper<M::Parameters>,
        rng: &mut impl Rng,
    ) -> ABCDResult<Generation<M::Parameters>> {
        let new_gen_number = prev_gen.generation_number() + 1;
        log::info!("Starting building generation #{}", new_gen_number);

        let tolerance = prev_gen.calculate_tolerance()?;
        let mut particle_failures = Vec::<String>::new();

        loop {
            // Particle loop
            if particle_failures.len() > self.config.algorithm.max_num_failures {
                return Err(ABCDErr::TooManyRetriesError(
                    "In particle loop".into(),
                    particle_failures,
                ));
            }

            let new_particle_result = self.make_one_particle(tolerance, prev_gen, rng);

            match new_particle_result {
                o @ Ok(_) => o,
                Err(ABCDErr::StaleGenerationErr(msg)) => return Err(ABCDErr::StaleGenerationErr(msg)),
                Err(e) => {
                    let msg = format!("In particle loop, failed to make particle: {}", e);
                    log::warn!("{}", msg);
                    particle_failures.push(msg);
                    continue;
                }
            }?;

            // Check if we now have the req'd num particles/reps, if so, break
            let num_accepted = self.storage.num_accepted_particles()?;
            if num_accepted < self.config.job.num_particles {
                log::info!("Accumulated {num_accepted} accepted particles in storage.");
            } else {
                break;
            }
        }

        self.flush_generation(tolerance, new_gen_number)
    }

    fn check_still_working_on_correct_generation(
        &self,
        prev_gen: &GenWrapper<M::Parameters>,
    ) -> ABCDResult<()> {
        let current = prev_gen.generation_number();
        let newest = self.storage.previous_gen_number()?;
        if newest != current {
            Err(ABCDErr::StaleGenerationErr(
                "We were building on gen {current}, but storage reports {newest} is now available."
                    .into(),
            ))
        } else {
            Ok(())
        }
    }

    fn make_one_particle(
        &self,
        tolerance: f64,
        prev_gen: &GenWrapper<M::Parameters>,
        rng: &mut impl Rng,
    ) -> ABCDResult<()> {
        let parameters = {
            // Step B3
            let sampled = prev_gen.sample(&self.model, rng);
            let perturbed = prev_gen.perturb(&sampled, &self.model, rng)?;
            // Step B4
            if self.model.prior_density(&perturbed) == 0.0 {
                Err(ABCDErr::ParticleErr(
                    "Perturbed particle out of prior bounds.".into(),
                ))
            } else {
                Ok(perturbed)
            }
        }?;
        log::info!("Proposed parameters:\n {:#?}", &parameters);

        let scores: ABCDResult<Vec<f64>> = (0..self.config.job.num_replicates)
            .map(|_| {
                self.check_still_working_on_correct_generation(prev_gen)?;
                // (B5a) run the model to get a score
                self.model.score(&parameters).map_err(|e|{
                    ABCDErr::SystemError(format!("Error in client model code: {e}"))
                })
            })
            .collect();
            
        let scores = scores?;

        log::info!("Scores = {:?}", &scores);

        // We now have a collection of scores for the particle
        // (B5b) Calculate f^hat by calc'ing proportion less than tolerance
        // (B6) Calculate not_normalised_weight for each particle from its f^hat (f^hat(p) * prior(p)) / denom)
        let particle: Particle<M::Parameters> =
            prev_gen.weigh(parameters, scores, tolerance, &self.model)?;

        // Save the non_normalised particle to storage
        match self.storage.save_particle(&particle) {
            Ok(_save_result) => ABCDResult::Ok(()),
            Err(e) => {
                let message = format!("Failed to save particle: {}", e);
                log::error!("{}", message);
                Err(e)
            }
        }
    }

    fn flush_generation(&self, tolerance: f64, new_gen_number: u16) -> ABCDResult<Generation<M::Parameters>> {
        // Load all the non_normalised particles
        let particles: Vec<Particle<M::Parameters>> = self.storage.load_accepted_particles()?;
        let rejections = self.storage.num_rejected_particles()?;
        let acceptance = {
            let num: f64 = cast::f64(particles.len());
            let rejected: f64 = cast::f64(rejections);
            (num / (num + rejected)) as f32
        };

        log::info!("Acceptance rate: {acceptance:.3}");
        log::info!("Tolerance: {tolerance:.3}");

        let new_generation = Generation::new(particles, new_gen_number, tolerance, acceptance);

        self.storage.save_new_gen(&new_generation)?;

        Ok(new_generation)
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
