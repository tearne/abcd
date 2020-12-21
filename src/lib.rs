pub trait Random { }

pub trait Model {
    type Parameters;

    fn prior_sample<R: Random>(&self, random: &R) -> Self::Parameters; //TODO check density of sampled value is 0
    fn prior_density(&self) -> f64;

    fn perturb(&self, p: Self::Parameters) -> Self::Parameters;
    fn pert_density(&self, a: Self::Parameters, b: Self::Parameters) -> f64;

    fn score(&self, p: Self::Parameters) -> f64;
}