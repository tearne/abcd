use rand::Rng;

pub mod olcm;

pub trait KernelBuilder<P, K> where K: Kernel<P>,
{
    fn build_kernel_around_particle(&self, params: &P) -> K;
}

pub trait Kernel<P> {
    fn perturb(&self, p: &P, rng: &mut impl Rng) -> P;
    fn pert_density(&self, from: &P, to: &P) -> f64;        
}

