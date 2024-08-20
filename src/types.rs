use std::{error::Error, fmt::Debug, ops::{Add, Sub}};

use nalgebra::DVector;
use rand::prelude::*;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use statrs::statistics::{Data, OrderStatistics};

use crate::{
    config::Config, error::{ABCDErr, ABCDResult}, kernel::{Kernel, KernelBuilder}, wrapper::GenWrapper
};

pub trait Model {
    type Parameters: Serialize + DeserializeOwned + Debug + Clone + TryFrom<DVector<f64>> + Into<DVector<f64>> + Add<Output = Self::Parameters> + Sub<Output = Self::Parameters>;
    type K: Kernel<Self::Parameters>;
    type Kb: KernelBuilder<Self::Parameters, Self::K>;

    fn prior_sample(&self, rng: &mut impl Rng) -> Self::Parameters;
    fn prior_density(&self, p: &Self::Parameters) -> f64;

    fn build_kernel_builder_for_generation(&self, prev_gen: &GenWrapper<Self::Parameters>) -> Result<&Self::Kb, Box<dyn Error>>;

    fn score(&self, p: &Self::Parameters) -> Result<f64, Box<dyn Error>>;
}


#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct Particle<P> {
    pub parameters: P,
    pub score: f64,
    pub weight: f64,
}


// //TODO split up
// pub trait Vector<const D: usize> 
// where 
//     Self: Sized
// {
//     fn to_column_vector(&self) -> SMatrix<f64, D, 1>;
//     fn from_column_vector(v: DVector<f64>) -> Result<Self, VectorConversionError>;
// }

// pub struct OLCM<const D: usize> {
//     pub mean: SMatrix<f64, D, 1>,
//     pub local_covariance: SMatrix<f64, D, D>,
//     pub distribution: MultivariateNormal,
// }
// impl<const D: usize> OLCM<D> {
//     pub fn new(mean: SMatrix<f64, D, 1>, local_covariance: SMatrix<f64, D, D>) -> ABCDResult<Self> {
//         //TODO better way?
//         let dynamic_d = mean.len();
//         let mean_dyn = DVector::from_vec(mean.iter().cloned().collect::<Vec<f64>>());
//         let cov_dyn = DMatrix::from_vec(
//             dynamic_d,
//             dynamic_d,
//             local_covariance.iter().cloned().collect::<Vec<f64>>(),
//         );

//         // cargo tree -i nalgebra@0.32.6
//         //TODO decouple nalgebra by passing in vec?
//         let distribution = MultivariateNormal::new_from_nalgebra(mean_dyn, cov_dyn)?;

//         Ok(Self {
//             mean,
//             local_covariance,
//             distribution,
//         })
//     }
// }

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
