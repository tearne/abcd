use std::marker::PhantomData;

use rand::Rng;

pub mod olcm;

pub trait KernelBuilder<P, K> where K: Kernel<P>,
{
    fn build_kernel_around_particle(&self, params: &P) -> &K;
}

pub trait Kernel<P> {
    fn perturb(&self, p: &P, rng: &mut impl Rng) -> P;
    fn pert_density(&self, from: &P, to: &P) -> f64;        
}

pub struct TrivialKernel<P, K: Kernel<P>> {
    kernel: K,
    phantom: PhantomData<P>,
}
impl<P,K: Kernel<P>> TrivialKernel<P, K> {
    pub fn from(kernel: K) -> Self {
        TrivialKernel { kernel, phantom: PhantomData::default() }
    }
}

impl<P,K: Kernel<P>> KernelBuilder<P, TrivialKernel<P, K>> for TrivialKernel<P, K> {
    fn build_kernel_around_particle(&self, _: &P) -> &TrivialKernel<P, K> {
        self
    }
}
impl<P, K: Kernel<P>> Kernel<P> for TrivialKernel<P, K>{
    fn perturb(&self, p: &P, rng: &mut impl Rng) -> P {
        self.kernel.perturb(p, rng)
    }
    
    fn pert_density(&self, from: &P, to: &P) -> f64 {
        self.kernel.pert_density(from, to)
    }
}

