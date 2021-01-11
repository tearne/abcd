mod storage;
mod error;

use serde::{Serialize, Deserialize};

pub trait Random { }

pub trait Model {
    type Parameters;

    fn prior_sample<R: Random>(&self, random: &R) -> Self::Parameters; //TODO check density of sampled value is 0
    fn prior_density(&self) -> f64;

    fn perturb(&self, p: Self::Parameters) -> Self::Parameters;
    fn pert_density(&self, a: Self::Parameters, b: Self::Parameters) -> f64;

    fn score(&self, p: Self::Parameters) -> f64;
}

#[derive(Serialize, Deserialize)]
struct Scored<P> {
    parameter: P,
    score: f64,
}
impl<P> Scored<P> {
    pub fn new(parameter: P, score: f64) -> Scored<P> {
        Scored{parameter, score}
    }
}

#[derive(Serialize, Deserialize)]
struct Weighted<P> {
    scored_vec: Vec<Scored<P>>,
    weight: f64,
}

#[derive(Serialize, Deserialize)]
struct Generation<P> {
    generation: u16,
    tolerance: f64,
    acceptance: f64,
    particles: Vec<Weighted<P>>,
}
