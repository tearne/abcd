use rand::prelude::*;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use statrs::statistics::{Data, Statistics, OrderStatistics};
use std::fmt::{Debug, Display};

use crate::{config::Config, error::{ABCDResult, ABCDErr}};

pub trait Model {
    type Parameters: Serialize + DeserializeOwned + Debug + Clone;
    type E: Display;

    fn prior_sample(&self, rng: &mut impl Rng) -> Self::Parameters;
    fn prior_density(&self, p: &Self::Parameters) -> f64;

    fn perturb(&self, p: &Self::Parameters, rng: &mut impl Rng) -> Self::Parameters;
    fn pert_density(&self, from: &Self::Parameters, to: &Self::Parameters) -> f64;

    fn score(&self, p: &Self::Parameters) -> Result<f64, Self::E>;
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct Particle<P> {
    pub parameters: P,
    pub scores: Vec<f64>,
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
        config: &Config
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
            next_gen_tolerance
        })
    }

    fn calculate_next_tolerance(particles: &[Particle<P>], config: &Config) -> ABCDResult<f64> {
        // Get distribution of scores from last generation then reduce by tolerance descent rate (configured) - crate exists for percentile =>
        let score_distribution: ABCDResult<Vec<f64>> = particles
            .iter()
            .map(|particle| {
                let mean_score: f64 = particle.scores.clone().mean();
                match mean_score.is_nan() {
                    false => Ok(mean_score),
                    true => Err(ABCDErr::SystemError("Mean score is not a number.".into())),
                }
            })
            .collect();

        let mut score_distribution = Data::new(score_distribution?);
        let new_tolerance =
            score_distribution.percentile(config.algorithm.tolerance_descent_percentile);

        match new_tolerance.is_nan() {
            false => {
                log::info!("Tolerance calculated as {new_tolerance}");
                Ok(new_tolerance)
            }
            true => Err(ABCDErr::SystemError(
                "Tolerance (from percentile) was not a number.".into(),
            )),
        }
    }
}
