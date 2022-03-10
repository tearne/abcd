use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::fmt::Debug;

use crate::Random;

pub trait Model {
    type Parameters: Serialize + DeserializeOwned + Debug;

    fn prior_sample(&self, random: &Random) -> Self::Parameters; //TODO check density of sampled value is NOT 0
    fn prior_density(&self, p: &Self::Parameters) -> f64;

    fn perturb(&self, p: &Self::Parameters) -> Self::Parameters;
    fn pert_density(&self, a: &Self::Parameters, b: &Self::Parameters) -> f64;

    fn score(&self, p: &Self::Parameters) -> f64;
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct Particle<P> {
    pub parameters: P,
    pub scores: Vec<f64>,
    pub weight: f64,
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub struct Population<P> {
    pub tolerance: f64,
    pub acceptance: f64,
    pub normalised_particles: Vec<Particle<P>>,
}
impl<P> Population<P> {
    pub fn new(tolterance: f64, acceptance: f64, normalised_particles: Vec<Particle<P>>) -> Self {
        todo!("Blow up if particles aren't normalised");
    }

    // pub fn tolerance(&self) -> f64 {
    //     self.tolerance
    // }

    // pub fn acceptance(&self) -> f64 {
    //     self.acceptance
    // }

    // pub fn particles(&self) -> &Vec<Particle<P>> {
    //     &self.normalised_particles
    // }
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct Generation<P> {
    pub pop: Population<P>,
    pub number: u16,
}
impl<P> Generation<P> {
    pub fn new(
        normalise_particles: P,
        generation_number: u16,
        tolerance: f64,
        acceptance: f64 //TODO change to an f16?
    ) -> Self{
        todo!()
    }
}
