use std::{
    marker::PhantomData,
    ops::{Add, Sub},
};

use nalgebra::SMatrix;
use rand::{distributions::Distribution, Rng};
use statrs::distribution::{Continuous, MultivariateNormal};

use crate::{error::ABCDResult, types::Vector, Particle};

use super::Kernel;

pub struct OLCMKernel<const D: usize, P>
where
    P: Vector<D> + Add<Output = P> + Sub<Output = P>,
{
    pub weighted_mean: SMatrix<f64, D, 1>,
    pub local_covariance: SMatrix<f64, D, D>,
    distribution: MultivariateNormal,
    phantom: PhantomData<P>,
}
impl<const D: usize, P> OLCMKernel<D, P>
where
    P: Vector<D> + Add<Output = P> + Sub<Output = P> + Copy,
{
    pub fn perturb(&self, parameters: &P, rng: &mut impl Rng) -> ABCDResult<P> {
        let sampled = P::from_column_vector(self.distribution.sample(rng))?;
        Ok(*parameters + sampled)
    }

    pub fn pert_density(&self, from: &P, to: &P) -> f64 {
        let delta: P = *to - *from;
        //TODO another case of SVector to DVector
        self.distribution.pdf(
            delta
                .to_column_vector()
                .iter()
                .cloned()
                .collect::<Vec<f64>>(),
        )
    }
}

impl<P> Kernel<P> for OLCMKernel<P> {
    fn perturb(&self, p: &P, rng: &mut impl Rng) -> P {
        self.perturb(p, rng)
    }

    fn pert_density(&self, from: &P, to: &P) -> f64 {
        self.pert_density(from, to)
    }
}

pub struct OLCMKernelBuilder<const D: usize, P>
where
    P: Vector<D> + Add<Output = P> + Sub<Output = P>,
{
    weighted_mean: SMatrix<f64, D, 1>,
    weighted_covariance: SMatrix<f64, D, D>,
    phantom: PhantomData<P>,
}
impl<const D: usize, P> OLCMKernelBuilder<D, P>
where
    P: Vector<D> + Add<Output = P> + Sub<Output = P>,
{
    pub fn new(particles: &Vec<Particle<P>>) -> ABCDResult<Self> {
        assert!(f64::abs(particles.iter().map(|p| p.weight).sum::<f64>() - 1.0) < 0.000001);

        let weighted_mean: SMatrix<f64, D, 1> =
            particles
                .iter()
                .fold(SMatrix::<f64, D, 1>::zeros(), |acc, particle| {
                    let parameters_vec = particle.parameters.to_column_vector();
                    let weight = particle.weight;
                    acc + weight * parameters_vec
                });

        let weighted_covariance: SMatrix<f64, D, D> =
            particles
                .iter()
                .fold(SMatrix::<f64, D, D>::zeros(), |acc, par| {
                    let params = par.parameters.to_column_vector();
                    let weight = par.weight;

                    acc + weight * (params - weighted_mean) * (params - weighted_mean).transpose()
                });

        Ok(Self {
            weighted_mean,
            weighted_covariance,
            phantom: PhantomData::default(),
        })
    }

    pub fn build_kernel_around(&self, particle: &Particle<P>) -> ABCDResult<OLCMKernel<D, P>> {
        let local_covariance = {
            let particle_vector = particle.parameters.to_column_vector();
            let bias = (self.weighted_mean - particle_vector)
                * (self.weighted_mean - particle_vector).transpose();
            self.weighted_covariance + bias
        };

        let distribution = MultivariateNormal::new(
            vec![0f64; D],
            local_covariance.iter().cloned().collect::<Vec<f64>>(),
        )?;

        Ok(OLCMKernel::<D, P> {
            weighted_mean: self.weighted_mean,
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
        types::Vector,
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
