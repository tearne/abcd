use std::{
    marker::PhantomData,
    ops::{Add, Sub},
};

use nalgebra::{dimension, DMatrix, DVector, SMatrix};
use rand::{distributions::Distribution, Rng};
use statrs::distribution::{Continuous, MultivariateNormal};

use crate::{error::{ABCDErr, ABCDResult}, Particle, ABCD};

use super::Kernel;

pub struct OLCMKernel<P>
where
    P: From<DVector<f64>> + Add<Output = P> + Sub<Output = P> + Copy,
{
    pub weighted_mean: DVector<f64>,
    pub local_covariance: DMatrix<f64>,
    distribution: MultivariateNormal,
    phantom: PhantomData<P>,
}
impl<P> OLCMKernel<P>
where
    P: From<DVector<f64>> + Into<DVector<f64>> + Add<Output = P> + Sub<Output = P> + Copy,
{
    pub fn perturb(&self, parameters: &P, rng: &mut impl Rng) -> P {
        let sampled: P = self.distribution.sample(rng).into();
        *parameters + sampled
    }

    pub fn pert_density(&self, from: &P, to: &P) -> f64 {
        let delta: P = *to - *from;
        let delta: DVector<f64> = delta.into();
        self.distribution.pdf(&delta)
    }
}

impl<P> Kernel<P> for OLCMKernel<P> 
where
    P: From<DVector<f64>> + Into<DVector<f64>> + Add<Output = P> + Sub<Output = P> + Copy
{
    fn perturb(&self, p: &P, rng: &mut impl Rng) -> P {
        self.perturb(p, rng)
    }

    fn pert_density(&self, from: &P, to: &P) -> f64 {
        self.pert_density(from, to)
    }
}

pub struct OLCMKernelBuilder<const D: usize, P>
where
    P: From<DVector<f64>> + Into<DVector<f64>> + Add<Output = P> + Sub<Output = P> + Copy,
{
    weighted_mean: DVector<f64>,
    weighted_covariance: DMatrix<f64>,
    phantom: PhantomData<P>,
}
impl<const D: usize, P> OLCMKernelBuilder<D, P>
where
    P: From<DVector<f64>> + Into<DVector<f64>> + Add<Output = P> + Sub<Output = P> + Copy,
{
    pub fn new(particles: &Vec<Particle<P>>) -> ABCDResult<Self> {
        assert!(f64::abs(particles.iter().map(|p| p.weight).sum::<f64>() - 1.0) < 0.000001);

        // let dimension = {
        //     let first_particle = particles.first().ok_or_else(||ABCDErr::OCLMError("Empty particle vector.".into()))?;
        //     let DVfirst_particle.

        // };

        let weighted_mean: DVector<f64> =
            particles
                .iter()
                .map(|particle|{
                    let parameters_vec = Into::<DVector<f64>>::into(particle.parameters);
                    let weight = particle.weight;
                    weight * parameters_vec
                })
                .reduce(|acc, vec| acc + vec)
                .ok_or_else(||ABCDErr::OCLMError("Failed to build weighted mean.".into()))?;

        let weighted_covariance: DMatrix<f64> =
            particles
                .iter()
                .map(|particle|{
                    let params = Into::<DVector<f64>>::into(particle.parameters);
                    let weight = particle.weight;
                    weight * (&params - &weighted_mean) * (&params - &weighted_mean).transpose()
                })
                .reduce(|acc, mat| acc + mat)
                .ok_or_else(||ABCDErr::OCLMError("Failed to build weighted covariance.".into()))?;

        Ok(Self {
            weighted_mean,
            weighted_covariance,
            phantom: PhantomData::default(),
        })
    }

    pub fn build_kernel_around(&self, particle: &Particle<P>) -> ABCDResult<OLCMKernel<P>> {
        let local_covariance = {
            let particle_vector = Into::<DVector<f64>>::into(particle.parameters);
            let bias = (&self.weighted_mean - &particle_vector)
                * (&self.weighted_mean - &particle_vector).transpose();
            &self.weighted_covariance + bias
        };

        let distribution = MultivariateNormal::new(
            vec![0f64; D],
            local_covariance.iter().cloned().collect::<Vec<f64>>(),
        )?;

        Ok(OLCMKernel::<P> {
            weighted_mean: self.weighted_mean.clone(),
            local_covariance,
            distribution,
            phantom: PhantomData::default(),
        })
    }
}

#[cfg(test)]
mod tests {
    use nalgebra::{DVector, Matrix2, SMatrix, Vector2};
    use serde::Deserialize;

    use crate::{
        error::{ABCDResult, VectorConversionError},
        kernel::olcm::OLCMKernelBuilder,
        Generation,
    };

    #[derive(Deserialize, Debug, derive_more::Add, derive_more::Sub)]
    struct TestParams {
        x: f64,
        y: f64,
    }

    impl Vector<2> for TestParams {
        fn to_column_vector(&self) -> SMatrix<f64, 2, 1> {
            Vector2::new(self.x, self.y)
        }

        fn from_column_vector(
            v: DVector<f64>,
        ) -> Result<TestParams, crate::error::VectorConversionError> {
            let values = v.iter().cloned().collect::<Vec<f64>>();
            if values.len() != 2 {
                return Err(VectorConversionError(format!(
                    "Wrong number of arguments.  Expected 2, got {}",
                    values.len()
                )));
            } else {
                Ok(TestParams {
                    x: values[0],
                    y: values[1],
                })
            }
        }
    }

    #[test]
    fn test_olcm() -> ABCDResult<()> {
        let path = "resources/test/olcm/particles.json";
        let generation: Generation<TestParams> =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();

        let normalised_particles = generation.pop.normalised_particles();
        let candidate = &normalised_particles[0];

        let olcm = OLCMKernelBuilder::new(normalised_particles)?.build_kernel_around(candidate)?;
        assert_eq!(Matrix2::new(4.8, -13.6, -13.6, 44.1), olcm.local_covariance);
        assert_eq!(Vector2::new(10.0, 100.1), olcm.weighted_mean);

        Ok(())
    }
}
