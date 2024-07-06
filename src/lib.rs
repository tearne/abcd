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
    // Run until the target generation as specified in config.job is met
    pub fn run(model: M, config: Config, storage: S, rng: &mut impl Rng) -> ABCDResult<()> {
        Self::inner_run(model, config, storage, rng, false)
    }

    // Run until the next generation is reached, then shut down
    pub fn boost(model: M, config: Config, storage: S, rng: &mut impl Rng) -> ABCDResult<()> {
        log::info!("Running in boost mode - will only run until the next generation.");
        Self::inner_run(model, config, storage, rng, true)
    }

    fn inner_run(
        model: M,
        config: Config,
        storage: S,
        rng: &mut impl Rng,
        boost_mode: bool,
    ) -> ABCDResult<()> {
        let abcd = ABCD {
            model,
            config,
            storage,
        };

        match abcd.generation_loop(rng, boost_mode) {
            Err(ABCDErr::StaleGenerationErr(msg)) | Err(ABCDErr::ParticleErr(msg)) => Err(
                ABCDErr::SystemError(format!("Unexpected error cascaded to top of ABCD: {}", msg)),
            ),
            other => other,
        }
    }

    fn generation_loop(&self, rng: &mut impl Rng, boost_mode: bool) -> ABCDResult<()> {
        let mut gen_failures = Vec::<String>::new();
        let start_gen_num = self.storage.previous_gen_number()?;

        loop {
            if gen_failures.len() > self.config.algorithm.max_num_failures {
                return Err(ABCDErr::TooManyRetriesError(
                    "Too many retries in generation loop".into(),
                    gen_failures,
                ));
            }

            let prev_gen_num_in_storage = self.storage.previous_gen_number()?;
            if prev_gen_num_in_storage >= self.config.job.num_generations
                && self.config.job.terminate_at_target_gen
            {
                log::info!(
                    "Reached target number of generations: {}",
                    prev_gen_num_in_storage
                );
                break;
            }

            let prev_gen = GenWrapper::<M::Parameters>::load_previous_gen::<M, S>(&self.storage)?;

            if boost_mode && prev_gen.generation_number() > start_gen_num {
                log::info!(
                    "Boost mode complete, from generation {} to {}",
                    start_gen_num,
                    prev_gen.generation_number()
                );
                break;
            }

            match self.make_particles_loop(&prev_gen, rng) {
                Ok(()) => (),
                Err(ABCDErr::StaleGenerationErr(msg)) => {
                    log::warn!("{}", msg);
                    gen_failures.push(msg);
                    continue;
                }
                Err(e) => {
                    let msg = format!("In generation loop, failed to make a new generation but will try again: {}", e);
                    log::error!("{}", msg);
                    gen_failures.push(msg);
                    continue;
                }
            };
        }

        Ok(())
    }

    fn make_particles_loop(
        &self,
        prev_gen: &GenWrapper<M::Parameters>,
        rng: &mut impl Rng,
    ) -> ABCDResult<()> {
        let new_gen_number = prev_gen.generation_number() + 1;
        log::info!("Start building generation #{}", new_gen_number);

        let mut particle_failures = Vec::<String>::new();
        
        // Build the kernel outside the loop, since it take a bit of effort
        let kernel: M::K = self.model.build_kernel(prev_gen)?;

        loop {
            // Particle loop
            if particle_failures.len() > self.config.algorithm.max_num_failures {
                return Err(ABCDErr::TooManyRetriesError(
                    "In particle loop".into(),
                    particle_failures,
                ));
            }

            self.check_still_working_on_correct_generation(prev_gen)?;

            match self.make_one_particle(prev_gen, &kernel, rng) {
                o @ Ok(_) => o,
                Err(e) => {
                    let msg = format!("In particle loop, failed to make particle: {}", e);
                    log::warn!("{}", msg);
                    particle_failures.push(msg);
                    continue;
                }
            }?;

            self.check_still_working_on_correct_generation(prev_gen)?;

            // Check if we now have the required num particles/reps, if so, break
            let num_accepted = self.storage.num_accepted_particles()?;
            if num_accepted < self.config.job.num_particles {
                if num_accepted % 10 == 0 {
                    log::info!("{num_accepted} accepted particles in storage.");
                }
            } else {
                break;
            }
        }

        self.flush_generation(new_gen_number)
    }

    fn check_still_working_on_correct_generation(
        &self,
        prev_gen: &GenWrapper<M::Parameters>,
    ) -> ABCDResult<()> {
        let current = prev_gen.generation_number();
        let newest = self.storage.previous_gen_number()?;
        if newest != current {
            Err(ABCDErr::StaleGenerationErr(format!(
                "We were building on gen {current}, but storage reports {newest} is now available."
            )))
        } else {
            Ok(())
        }
    }

    fn make_one_particle(
        &self,
        prev_gen: &GenWrapper<M::Parameters>,
        kernel: &M::K,
        rng: &mut impl Rng,
    ) -> ABCDResult<()> {
        let parameters = {
            // Sample from previous generation
            let sampled = prev_gen.sample(&self.model, rng);
            // Apply perturbation kernel
            let perturbed = prev_gen.perturb(&sampled, &self.model, rng)?;
            // Ensure perturbed particle is within the prior, else will try again
            if self.model.prior_density(&perturbed) == 0.0 {
                Err(ABCDErr::ParticleErr(
                    "Perturbed particle out of prior bounds.".into(),
                ))
            } else {
                Ok(perturbed)
            }
        }?;
        log::debug!("Proposed parameters:\n {:#?}", &parameters);

        // Run model to calculate a score (now only one rep)
        let score: f64 = self
            .model
            .score(&parameters)
            .map_err(|e| ABCDErr::SystemError(format!("Error in client model code: {e}")))?;

        log::debug!("Score = {:?}", &score);

        // Calculate not_normalised_weight based on score (zero if score < threshold)
        let particle: Particle<M::Parameters> = prev_gen.weigh(
            parameters,
            score,
            prev_gen.next_gen_tolerance()?,
            &self.model,
        )?;

        // Save the non_normalised particle to storage
        let save_as_gen = prev_gen.generation_number() + 1;
        match self.storage.save_particle(&particle, save_as_gen) {
            Ok(_save_result) => ABCDResult::Ok(()),
            Err(e) => {
                let message = format!("Failed to save particle: {}", e);
                log::error!("{}", message);
                Err(e)
            }
        }
    }

    fn flush_generation(&self, new_gen_number: u16) -> ABCDResult<()> {
        // Load all the non_normalised particles
        let particles: Vec<Particle<M::Parameters>> = self.storage.load_accepted_particles()?;
        let rejections = self.storage.num_rejected_particles()?;
        let acceptance = {
            let num: f64 = cast::f64(particles.len());
            let rejected: f64 = cast::f64(rejections);
            (num / (num + rejected)) as f32
        };

        let new_generation = Generation::new(particles, new_gen_number, acceptance, &self.config)?;

        log::info!("Acceptance rate: {acceptance:.3}");
        log::info!(
            "Next gen tolerance: {:.3}",
            new_generation.next_gen_tolerance
        );

        self.storage.save_new_gen(&new_generation)?;

        Ok(())
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
