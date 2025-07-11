use std::{borrow::Cow, error::Error, fmt::Debug, ops::{Add, Sub}};

use nalgebra::DVector;
use rand::prelude::*;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use statrs::statistics::{Data, OrderStatistics};

use crate::{
    config::AbcdConfig, error::{ABCDErr, ABCDResult}, kernel::{Kernel, KernelBuilder}
};

pub trait Model {
    type Parameters: Serialize + DeserializeOwned + Debug + Clone + TryFrom<DVector<f64>> + Into<DVector<f64>> + Add<Output = Self::Parameters> + Sub<Output = Self::Parameters>;
    type K: Kernel<Self::Parameters>;
    type Kb: KernelBuilder<Self::Parameters, Self::K>;

    fn prior_sample(&self, rng: &mut impl Rng) -> Self::Parameters;
    fn prior_density(&self, p: &Self::Parameters) -> f64;

    fn build_kernel_builder<'a>(&'a self, prev_gen_particles: &Vec<Particle<Self::Parameters>>) -> Result<Cow<'a, Self::Kb>, Box<dyn Error>>;

    fn score(&self, p: &Self::Parameters) -> Result<f64, Box<dyn Error>>;
}


#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct Particle<P> {
    pub parameters: P,
    pub score: f64,
    pub weight: f64,
}


#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub struct Population<P> {
    acceptance: f32,
    normalised_particles: Vec<Particle<P>>,
}
impl<P> Population<P> {
    pub fn new(normalised_particles: Vec<Particle<P>>, acceptance: f32) -> Self {
        Self {
            acceptance,
            normalised_particles,
        }
    }

    pub fn normalised_particles(&self) -> &Vec<Particle<P>> {
        &self.normalised_particles
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct Generation<P> {
    pub pop: Population<P>,
    pub number: u16,
    pub next_gen_tolerance: f64,
}
impl<P> Generation<P> {
    pub fn new(
        mut particles: Vec<Particle<P>>,
        generation_number: u16,
        acceptance: f32,
        config: &AbcdConfig,
    ) -> ABCDResult<Self> {
        let total_weight: f64 = particles.iter().map(|p| p.weight).sum();

        //(B7) Normalise all the weights together
        #[allow(clippy::assign_op_pattern)]
        particles
            .iter_mut()
            .for_each(|p| p.weight = p.weight / total_weight);

        let next_gen_tolerance = Self::calculate_next_tolerance(&particles, config)?;

        Ok(Self {
            pop: Population::<P>::new(particles, acceptance),
            number: generation_number,
            next_gen_tolerance,
        })
    }

    fn calculate_next_tolerance(particles: &[Particle<P>], config: &AbcdConfig) -> ABCDResult<f64> {
        // Get distribution of scores from last generation then reduce by tolerance descent rate (configured) - crate exists for percentile =>
        let score_distribution: ABCDResult<Vec<f64>> = particles
            .iter()
            .map(|particle| {
                let score: f64 = particle.score;
                match score >= 0.0 {
                    true => Ok(score),
                    false => Err(ABCDErr::SystemError(format!(
                        "Encountered negative score ({}) when calculating new tolerance.",
                        score
                    ))),
                }
            })
            .collect();

        let mut score_distribution = Data::new(score_distribution?);
        let new_tolerance =
            score_distribution.percentile(config.tolerance_descent_percentile);

        match new_tolerance.is_nan() {
            false => {
                log::info!("New tolerance: {new_tolerance}");
                Ok(new_tolerance)
            }
            true => Err(ABCDErr::SystemError(
                "Tolerance (from percentile) was not a number (NaN).".into(),
            )),
        }
    }
}
