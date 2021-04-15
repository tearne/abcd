mod storage;
mod error;

use serde::{Serialize, Deserialize};
use std::cmp::Ordering;

pub trait Random { }

pub trait Model {
    type Parameters;

    fn prior_sample<R: Random>(&self, random: &R) -> Self::Parameters; //TODO check density of sampled value is NOT 0
    fn prior_density(&self, p: Self::Parameters) -> f64;

    fn perturb(&self, p: Self::Parameters) -> Self::Parameters;
    fn pert_density(&self, a: Self::Parameters, b: Self::Parameters) -> f64;

    fn score(&self, p: Self::Parameters) -> f64;
}

// #[derive(Serialize, Deserialize, Debug, PartialEq)]
// struct Scored<P> {
//     parameters: P,
//     score: f64,
// }
// impl<P> Scored<P> {
//     pub fn new(parameters: P, score: f64) -> Scored<P> {
//         Scored{parameters, score}
//     }
// }

//TODO rename to particle?
#[derive(Serialize, Deserialize, Debug, PartialEq)]
struct Weighted<P> {
    parameters: P,
    scores: Vec<f64>,
    weight: f64,
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
struct Generation<P> {
    generation_number: u16,
    tolerance: f64,
    acceptance: f64,
    particles: Vec<Weighted<P>>,
}

