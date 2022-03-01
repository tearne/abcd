mod algorithm;
mod error;
mod etc;
mod storage;
mod types;

use std::{thread::Thread, convert::TryInto};

use error::{ABCDError, ABCDResult};
use etc::config::Config;
use rand::distributions::WeightedIndex;
use rand::prelude::*;
use storage::Storage;
pub use types::{ Model, Generation, Particle};

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
            ActualGeneration::new(gen),
        )?;
        if number == config.job.num_generations && config.job.terminate_at_target_gen {
            break;
        }
    }

    Ok(())
}

trait GenerationStuff<M: Model> {
    fn propose_me_a_parmeter_set(&self, model: &M, random: &ThreadRng) -> M::Parameters;
    fn calculate_me_a_tolerance(&self) -> f64;
    fn weigh_me_a_particle(&self, scores: Vec<f64>, model: &M, tolerance: f64) -> Option<Particle<M::Parameters>>;
}
struct ActualGeneration<P>{
    gen: Generation<P>,
}
impl<P> ActualGeneration<P> {
    fn new(gen: Generation<P>) -> Self {
        Self { gen }
    }
}
impl<M: Model> GenerationStuff<M> for ActualGeneration<M::Parameters>{
    fn propose_me_a_parmeter_set(&self, model: &M, random: &ThreadRng) -> <M as Model>::Parameters {
        todo!()
    }

    fn calculate_me_a_tolerance(&self) -> f64 {
        todo!()
    }

    fn weigh_me_a_particle(&self, scores: Vec<f64>, model: &M, tolerance: f64) -> Option<Particle<<M as Model>::Parameters>> {
        todo!()
    }
}
struct PriorGeneration{}
impl<M: Model> GenerationStuff<M> for PriorGeneration{
    fn propose_me_a_parmeter_set(&self, model: &M, random: &ThreadRng) -> <M as Model>::Parameters {
        todo!()
    }

    fn calculate_me_a_tolerance(&self) -> f64 {
        f64::MAX
    }

    fn weigh_me_a_particle(&self, scores: Vec<f64>, model: &M, tolerance: f64) -> Option<Particle<<M as Model>::Parameters>> {
        todo!()
    }
}

// struct PriorProposer { //TODO rename to reflect fact that it does two things (a) proposing, (b) weighing

// }
// impl PriorProposer {
//     pub fn new() -> Self {
//         PriorProposer {}
//     }
// }
// impl<M: Model> Proposer<M> for PriorProposer {
//     fn next(&self, model: &M, random: &mut Random) -> <M as Model>::Parameters {
//         model.prior_sample(random)
//     }
// }

// struct PreviousGenerationProposer<P> {
//     generation: Generation<P>,
// }
// impl<P> PreviousGenerationProposer<P> {
//     pub fn new(generation: Generation<P>) -> Self {
//         PreviousGenerationProposer { generation }
//     }
// }
// impl<M: Model> Proposer<M> for PreviousGenerationProposer<M::Parameters> {
//     fn next(&self, model: &M, random: &mut Random) -> <M as Model>::Parameters {
//         sample_and_perturb_with_support(&self.generation, model, random)
//     }
// }

fn do_gen<M: Model, S: Storage>(
    storage: &S,
    model: &M,
    config: &Config,
    random: &mut Random,
    gen_stuff: impl GenerationStuff<M>,
) -> ABCDResult<u16> {
    let prev_gen_number = storage.previous_gen_number()?;
    let tolerance = gen_stuff.calculate_me_a_tolerance();
    loop {
        //Particle loop

        // Particle loop
        // TODO loop could go on forever?  Use some kind of timeout, or issue warning?
        // (B3) sample a (fitting) parameter set from gen (perturb based on weights and kernel if sampling from generation)
        // (B4) Check if prior probability is zero - if so sample again
        let parameters: <M as Model>::Parameters = gen_stuff.propose_me_a_parmeter_set(model, random);

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
        let particle = gen_stuff.weigh_me_a_particle(scores, model, tolerance);
        // let particle = algorithm::weigh_particle(scores, f64::MAX, model, prev_gen_number);
        // let particle = Particle{
        //     parameters,
        //     scores,
        //     weight,
        // };
        let accepted = match particle {
            None => false,
            Some(_) => true
        };

        // Save the non_normalised particle to storage
        if accepted { storage.save_particle(&particle.unwrap())?; } //Was passing option in to save_particle and doing work there but decided not to.
        else {storage.save_rejected_particle();} //Can't save rejected particle contents if passed back as None above - unless there is an alternative? Also can't save rejected paricle files in same place as accepted - otherwise it screws up counting of accepted particle files

        // Check if we now have the req'd num particles/reps, if so, break
        if storage.num_working_particles()? >= config.job.num_particles {
            // Load all the non_normalised particles
            let particles: Vec<Particle<M::Parameters>> = storage.load_current_accepted_particles()?;
            let rejections = storage.count_current_rejected_particles()?;
            let acceptance = {
                let num: f64 = cast::f64(particles.len()); //TODO check we understand this, seems to be infallable??!
                let rejected: f64 =  cast::f64(rejections);
                num / (num + rejected)
            };

            // (B7) Normalise all the weights together
            let normalised = algorithm::normalise(particles);
            let new_generation = Generation::new(normalised, prev_gen_number + 1, tolerance, acceptance);

            // Save generation to storage
            storage.save_new_gen(&new_generation);

            return Ok(new_generation.number);
        }
    }
}


//TODO put this in the previous gen proposer
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
