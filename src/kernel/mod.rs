use std::{borrow::Cow, marker::PhantomData};

use rand::Rng;

use crate::error::ABCDResult;

pub mod olcm;

pub trait KernelBuilder<P, K>: Clone
where
    K: Kernel<P>,
{
    fn build_kernel_around_parameters<'a>(&'a self, params: &P) -> ABCDResult<Cow<'a, K>>;
}

pub trait Kernel<P>: Clone {
    fn perturb(&self, p: &P, rng: &mut impl Rng) -> ABCDResult<P>;
    fn pert_density(&self, from: &P, to: &P) -> f64;
}

#[derive(Clone)]
pub struct TrivialKernel<P, K: Kernel<P>> {
    kernel: K,
    phantom: PhantomData<P>,
}
impl<P, K: Kernel<P>> TrivialKernel<P, K> {
    pub fn from(kernel: K) -> Self {
        TrivialKernel {
            kernel,
            phantom: PhantomData::default(),
        }
    }
}

impl<P: Clone, K: Kernel<P>> KernelBuilder<P, TrivialKernel<P, K>> for TrivialKernel<P, K> {
    fn build_kernel_around_parameters<'a>(
        &'a self,
        _: &P,
    ) -> ABCDResult<Cow<'a, TrivialKernel<P, K>>> {
        Ok(Cow::Borrowed(self))
    }
}

impl<P: Clone, K: Kernel<P>> Kernel<P> for TrivialKernel<P, K> {
    fn perturb(&self, p: &P, rng: &mut impl Rng) -> ABCDResult<P> {
        self.kernel.perturb(p, rng)
    }

    fn pert_density(&self, from: &P, to: &P) -> f64 {
        self.kernel.pert_density(from, to)
    }
}
