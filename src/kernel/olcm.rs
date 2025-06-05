use std::{
    borrow::Cow,
    marker::PhantomData,
    ops::{Add, Sub},
};

use nalgebra::{DMatrix, DVector};
use rand::{distributions::Distribution, Rng};
use statrs::distribution::{Continuous, MultivariateNormal};

use crate::{
    error::{ABCDErr, ABCDResult},
    Particle,
};

use super::{Kernel, KernelBuilder};

#[derive(Clone)]
pub struct OLCMKernel<P>
where
    P: TryFrom<DVector<f64>, Error = ABCDErr> + Add<Output = P> + Sub<Output = P> + Copy,
{
    pub weighted_mean: DVector<f64>,
    pub local_covariance: DMatrix<f64>,
    distribution: MultivariateNormal,
    phantom: PhantomData<P>,
}
impl<P> OLCMKernel<P>
where
    P: TryFrom<DVector<f64>, Error = ABCDErr>
        + Into<DVector<f64>>
        + Add<Output = P>
        + Sub<Output = P>
        + Copy,
{
    pub fn perturb(&self, parameters: &P, rng: &mut impl Rng) -> ABCDResult<P> {
        let sampled: P = self.distribution.sample(rng).try_into()?;
        Ok(*parameters + sampled)
    }

    pub fn pert_density(&self, from: &P, to: &P) -> f64 {
        let delta: P = *to - *from;
        let delta: DVector<f64> = delta.into();
        self.distribution.pdf(&delta)
    }
}

impl<P> Kernel<P> for OLCMKernel<P>
where
    P: TryFrom<DVector<f64>, Error = ABCDErr>
        + Into<DVector<f64>>
        + Add<Output = P>
        + Sub<Output = P>
        + Copy,
{
    fn perturb(&self, p: &P, rng: &mut impl Rng) -> ABCDResult<P> {
        self.perturb(p, rng)
    }

    fn pert_density(&self, from: &P, to: &P) -> f64 {
        self.pert_density(from, to)
    }
}

#[derive(Clone)]
pub struct OLCMKernelBuilder<P>
where
    P: TryFrom<DVector<f64>, Error = ABCDErr>
        + Into<DVector<f64>>
        + Add<Output = P>
        + Sub<Output = P>
        + Copy,
{
    weighted_mean: DVector<f64>,
    weighted_covariance: DMatrix<f64>,
    phantom: PhantomData<P>,
}
impl<P> OLCMKernelBuilder<P>
where
    P: TryFrom<DVector<f64>, Error = ABCDErr>
        + Into<DVector<f64>>
        + Add<Output = P>
        + Sub<Output = P>
        + Copy,
{
    pub fn new(particles: &Vec<Particle<P>>) -> ABCDResult<Self> {
        assert!(
            f64::abs(particles.iter().map(|p| p.weight).sum::<f64>() - 1.0) < 0.000001,
            "Particles must be normalised to build OLCM kernel builder."
        );

        let weighted_mean: DVector<f64> = particles
            .iter()
            .map(|particle| {
                let parameters_vec = Into::<DVector<f64>>::into(particle.parameters);
                let weight = particle.weight;
                weight * parameters_vec
            })
            .reduce(|acc, vec| acc + vec)
            .ok_or_else(|| ABCDErr::OCLMError("Failed to build weighted mean.".into()))?;

        let mut weighted_cov: DMatrix<f64> = particles
            .iter()
            .map(|particle| {
                let params = Into::<DVector<f64>>::into(particle.parameters);
                let weight = particle.weight;
                weight * (&params - &weighted_mean) * (&params - &weighted_mean).transpose()
            })
            .reduce(|acc, mat| acc + mat)
            .ok_or_else(|| ABCDErr::OCLMError("Failed to build weighted covariance.".into()))?;

        let diffs = weighted_cov.lower_triangle() - weighted_cov.upper_triangle().transpose();
        if diffs.max() < f64::EPSILON {
            // Cov matrix should be symmetric but calculations may be imprecise due to floating point multiplication
            make_symmetric(&mut weighted_cov)?;
        } else {
            ABCDErr::OCLMError("weighted covariance matrix is not symmetric".into());
        }

        Ok(Self {
            weighted_mean,
            weighted_covariance: weighted_cov,
            phantom: PhantomData::default(),
        })
    }

    pub fn build_kernel(&self, parameters: &P) -> ABCDResult<OLCMKernel<P>> {
        let local_covariance = {
            let particle_vector = Into::<DVector<f64>>::into(*parameters);
            let bias = (&self.weighted_mean - &particle_vector)
                * (&self.weighted_mean - &particle_vector).transpose();
            &self.weighted_covariance + bias
        };

        let distribution = MultivariateNormal::new_from_nalgebra(
            DVector::<f64>::zeros(self.weighted_mean.len()),
            local_covariance.clone(),
        )?;

        Ok(OLCMKernel::<P> {
            weighted_mean: self.weighted_mean.clone(),
            local_covariance,
            distribution,
            phantom: PhantomData::default(),
        })
    }
}

impl<P> KernelBuilder<P, OLCMKernel<P>> for OLCMKernelBuilder<P>
where
    P: TryFrom<DVector<f64>, Error = ABCDErr>
        + Into<DVector<f64>>
        + Add<Output = P>
        + Sub<Output = P>
        + Copy,
{
    fn build_kernel_around_parameters<'a>(
        &'a self,
        parameters: &P,
    ) -> ABCDResult<Cow<'a, OLCMKernel<P>>> {
        Ok(Cow::Owned(self.build_kernel(parameters)?))
    }
}

#[cfg(test)]
mod tests {
    use nalgebra::{DVector, Matrix2, Vector2};
    use serde::Deserialize;

    use crate::{
        error::{ABCDErr, ABCDResult},
        kernel::{olcm::OLCMKernelBuilder, KernelBuilder},
        Generation,
    };

    #[derive(Deserialize, Debug, derive_more::Add, derive_more::Sub, Copy, Clone)]
    struct TestParams {
        x: f64,
        y: f64,
    }

    impl TryFrom<DVector<f64>> for TestParams {
        type Error = ABCDErr;

        fn try_from(value: DVector<f64>) -> Result<Self, Self::Error> {
            if value.len() != 2 {
                return Err(ABCDErr::VectorConversionError(format!(
                    "Wrong number of arguments.  Expected 2, got {}",
                    value.len()
                )));
            } else {
                Ok(TestParams {
                    x: value[0],
                    y: value[1],
                })
            }
        }
    }

    impl Into<DVector<f64>> for TestParams {
        fn into(self) -> DVector<f64> {
            DVector::from_column_slice(&[self.x, self.y])
        }
    }

    #[test]
    fn test_olcm() -> ABCDResult<()> {
        let path = "resources/test/olcm/particles.json";
        let generation: Generation<TestParams> =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();

        let normalised_particles = generation.pop.normalised_particles();
        let candidate = &normalised_particles[0].parameters;

        let kernel_builder = OLCMKernelBuilder::<TestParams>::new(normalised_particles)?;
        let olcm = kernel_builder.build_kernel_around_parameters(candidate)?;
        assert_eq!(Matrix2::new(4.8, -13.6, -13.6, 44.1), olcm.local_covariance);
        assert_eq!(Vector2::new(10.0, 100.1), olcm.weighted_mean);

        Ok(())
    }
}

fn make_symmetric(matrix: &mut DMatrix<f64>) -> ABCDResult<()> {
    if matrix.nrows() == 0 {
        return Ok(()); // Nothing to do for an empty matrix
    }

    if matrix.ncols() != matrix.nrows() {
        ABCDErr::OCLMError("Matrix must be square to be symmetric.".into());
    }

    // Iterate through the upper triangle (including diagonal)
    for i in 0..matrix.nrows() {
        for j in i..matrix.ncols() {
            // Ensure symmetry: mirror the upper triangle to the lower triangle
            matrix[(j, i)] = matrix[(i, j)];
        }
    }

    // Check that matrix is now symmetric
    if matrix.lower_triangle() != matrix.upper_triangle().transpose() {
        ABCDErr::OCLMError("weighted covariance matrix is still not symmetric".into());
    }

    Ok(())
}
