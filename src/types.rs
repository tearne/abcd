use nalgebra::{DVector, SMatrix};
use rand::prelude::*;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use statrs::{
    distribution::MultivariateNormal,
    statistics::{Data, OrderStatistics},
};
use std::{fmt::{Debug, Display}, marker::PhantomData};

use crate::{
    config::Config,
    error::{ABCDErr, ABCDResult, VectorConversionError}, wrapper::GenWrapper,
};

pub trait Kernel<P> where 
        P: Serialize + DeserializeOwned + Debug + Clone {

    fn perturb(&self, p: &P, rng: &mut impl Rng) -> P;
    fn pert_density(&self, from: &P, to: &P) -> f64;        
}

struct OLCMKernel<const D: usize, P: Vector<D>> {
    pub weighted_mean:  SMatrix<f64, D, 1>,
    weighted_covariance: SMatrix<f64, D, D>,
    phantom: PhantomData<P>,
}
impl<const D: usize, P: Vector<D>> OLCMKernel<D, P> {
    pub fn new(particles: &Vec<Particle<P>>) -> ABCDResult<Self> {
        assert!(f64::abs(particles.iter().map(|p|p.weight).sum::<f64>() - 1.0) < 0.000001);

        let weighted_mean: SMatrix<f64, D, 1> = particles.iter().fold(
            SMatrix::<f64, D, 1>::zeros(),
            |acc, particle| {
                let parameters_vec = particle.parameters.to_column_vector();
                let weight = particle.weight;
                acc + weight * parameters_vec
            },
        );

        let weighted_covariance: SMatrix<f64, D, D> =
            particles.iter()
                .fold(SMatrix::<f64, D, D>::zeros(), |acc, par| {
                    let params = par.parameters.to_column_vector();
                    let weight = par.weight;

                    acc + weight * (params - weighted_mean) * (params - weighted_mean).transpose()
                });

        Ok(OLCMKernel{
            weighted_mean,
            weighted_covariance,
            phantom: PhantomData::default(),
        })
    }

    pub fn local_covariance_matrix(&self, particle: &Particle<P>) -> SMatrix<f64, D, D> {
        let particle_vector = particle.parameters.to_column_vector();
        let bias = (self.weighted_mean - particle_vector) * (self.weighted_mean - particle_vector).transpose();
        self.weighted_covariance + bias
    }

    pub fn perturb(&self, particle: &Particle<P>, rng: &mut impl Rng) -> ABCDResult<P> {
        let local_covariance = self.local_covariance_matrix(particle);

        //TODO cheap way to convert from SMatrix to DMatrix?
        let distribution = MultivariateNormal::new(
            vec![0f64; D], 
            local_covariance.iter().cloned().collect::<Vec<f64>>()
        )?;

        let sampled = distribution.sample(rng);
        Ok(P::from_column_vector(sampled)?)
    }
}

pub trait Model {
    type Parameters: Serialize + DeserializeOwned + Debug + Clone;
    type K: Kernel<Self::Parameters>;
    type Err: Display;

    fn prior_sample(&self, rng: &mut impl Rng) -> Self::Parameters;
    fn prior_density(&self, p: &Self::Parameters) -> f64;

    fn build_kernel(self, prev_gen: &GenWrapper<Self::Parameters>) -> ABCDResult<Self::K>;
    // fn perturb(&self, p: &Self::Parameters, rng: &mut impl Rng) -> Self::Parameters;
    // fn pert_density(&self, from: &Self::Parameters, to: &Self::Parameters) -> f64;

    fn score(&self, p: &Self::Parameters) -> Result<f64, Self::Err>;
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct Particle<P> {
    pub parameters: P,
    pub score: f64,
    pub weight: f64,
}

//TODO split up
pub trait Vector<const D: usize> 
where 
    Self: Sized
{
    fn to_column_vector(&self) -> SMatrix<f64, D, 1>;
    fn from_column_vector(v: DVector<f64>) -> Result<Self, VectorConversionError>;
}

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

    // pub fn olcm<const D: usize>(&self, locality: &Particle<P>) -> ABCDResult<OLCM<D>>
    // where
    //     P: Vector<D>,
    // {
    //     let mean: SMatrix<f64, D, 1> = self.normalised_particles().iter().fold(
    //         SMatrix::<f64, D, 1>::zeros(),
    //         |acc, particle| {
    //             let parameters_vec = particle.parameters.to_column_vector();
    //             let weight = particle.weight;
    //             acc + weight * parameters_vec
    //         },
    //     );

    //     let candidate = locality.parameters.to_column_vector();

    //     let cov: SMatrix<f64, D, D> =
    //         self.normalised_particles
    //             .iter()
    //             .fold(SMatrix::<f64, D, D>::zeros(), |acc, par| {
    //                 let params = par.parameters.to_column_vector();
    //                 let weight = par.weight;

    //                 acc + weight * (params - mean) * (params - mean).transpose()
    //             });

    //     let bias = (mean - candidate) * (mean - candidate).transpose();
    //     let local_covariance = cov + bias;

    //     assert!(cov.upper_triangle().transpose() == cov.lower_triangle());

    //     OLCM::new(mean,local_covariance)
    // }
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

#[cfg(test)]
mod tests {
    use nalgebra::{DVector, Matrix2, SMatrix, Vector2};
    use serde::Deserialize;

    use crate::{error::{ABCDResult, VectorConversionError}, types::{OLCMKernel, Vector}, Generation};

    #[derive(Deserialize, Debug)]
    struct TestParams{
        x: f64,
        y: f64,
    }
    
    impl Vector<2> for TestParams {
        fn to_column_vector(&self) -> SMatrix<f64, 2, 1> {
           Vector2::new(self.x, self.y)
        }
       
        fn from_column_vector(v: DVector<f64>) -> Result<TestParams, crate::error::VectorConversionError> {
            let values = v.iter().cloned().collect::<Vec<f64>>();
            if values.len() != 2 {
                return Err(VectorConversionError(format!("Wrong number of arguments.  Expected 2, got {}", values.len())));
            } else {
                Ok(TestParams{
                    x: values[0],
                    y: values[1]
                })
            }
        }
    }

    #[test]
    fn test_olcm() -> ABCDResult<()> {
        let path = "resources/test/olcm/particles.json";
        let generation: Generation<TestParams> = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();

        let normalised_particles = generation.pop.normalised_particles;
        let candidate = &normalised_particles[0];

        let olcm = OLCMKernel::new(generation.pop.normalised_particles())?;
        assert_eq!(Matrix2::new(4.8, -13.6, -13.6, 44.1), olcm.local_covariance_matrix(candidate));
        assert_eq!(Vector2::new(10.0, 100.1), olcm.weighted_mean);

        Ok(())
    }
}