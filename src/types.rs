use nalgebra::{DMatrix, DVector, SMatrix};
use rand::prelude::*;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use statrs::{distribution::MultivariateNormal, statistics::{Data, OrderStatistics}};
use std::fmt::{Debug, Display};

use crate::{
    config::Config,
    error::{ABCDErr, ABCDResult},
};

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
    pub score: f64,
    pub weight: f64,
}

pub trait Vectorable<const D: usize>{
    fn to_column_vector(&self) -> SMatrix<f64, D, 1>;
}

pub struct OLCM<const D: usize>{
    pub mean: SMatrix<f64, D, 1>,
    pub local_covariance: SMatrix<f64, D, D>,
}
impl<const D: usize> OLCM<D> {
    pub fn distribution(&self) -> ABCDResult<MultivariateNormal> {
        //TODO better way?
        let dynamic_d = self.mean.len();
        let mean = DVector::from_vec(self.mean.iter().cloned().collect::<Vec<f64>>());
        let cov = DMatrix::from_vec(dynamic_d, dynamic_d, self.local_covariance.iter().cloned().collect::<Vec<f64>>());

        // cargo tree -i nalgebra@0.32.6
        //TODO decouple by passing in vec
        let mvn = MultivariateNormal::new_from_nalgebra(mean, cov)?;

        Ok(mvn)
    }
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

    pub fn olcm<const D: usize>(&self, locality: &Particle<P>) -> OLCM<D> 
    where
        P: Vectorable<D>,
    {
        let mean: SMatrix<f64, D, 1> = self.normalised_particles().iter().fold(SMatrix::<f64, D, 1>::zeros(), |acc, particle|{
            let parameters_vec = particle.parameters.to_column_vector();
            let weight = particle.weight;
            acc + weight * parameters_vec
        });

        let candidate = locality.parameters.to_column_vector();

        let cov: SMatrix<f64, D, D> = self.normalised_particles.iter().fold(SMatrix::<f64, D, D>::zeros(), |acc, par|{
            let params = par.parameters.to_column_vector();
            let weight = par.weight;

            acc + weight * (params - mean) * (params - mean).transpose()
        });
        
        let bias = (mean - candidate) * (mean - candidate).transpose();
        let local_covariance = cov + bias;
        
        assert!(cov.upper_triangle().transpose() == cov.lower_triangle());

        OLCM{
            local_covariance,
            mean,
        }
        
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
        config: &Config,
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

    fn calculate_next_tolerance(particles: &[Particle<P>], config: &Config) -> ABCDResult<f64> {
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
            score_distribution.percentile(config.algorithm.tolerance_descent_percentile);

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
