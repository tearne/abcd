use rand::prelude::*;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::fmt::Debug;

pub trait Model {
    type Parameters: Serialize + DeserializeOwned + Debug + Clone;

    fn prior_sample(&self, rng: &mut impl Rng) -> Self::Parameters;
    fn prior_density(&self, p: &Self::Parameters) -> f64;

    fn perturb(&self, p: &Self::Parameters, rng: &mut impl Rng) -> Self::Parameters;
    fn pert_density(&self, from: &Self::Parameters, to: &Self::Parameters) -> f64;

    fn score<E: std::error::Error>(&self, p: &Self::Parameters) -> Result<f64, E>;
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct Particle<P> {
    pub parameters: P,
    pub scores: Vec<f64>,
    pub weight: f64,
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub struct Population<P> {
    tolerance: f64,
    acceptance: f32,
    normalised_particles: Vec<Particle<P>>,
}
impl<P> Population<P> {
    pub fn new(tolerance: f64, acceptance: f32, normalised_particles: Vec<Particle<P>>) -> Self {
        Self {
            tolerance,
            acceptance,
            normalised_particles
        }
    }

    pub fn normalised_particles(&self) -> &Vec<Particle<P>> {
        &self.normalised_particles
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct Generation<P> {
    pub pop: Population<P>,
    pub number: u16,
}
impl<P> Generation<P> {
    pub fn new(
        mut particles: Vec<Particle<P>>,
        generation_number: u16,
        tolerance: f64,
        acceptance: f32
    ) -> Self{
        let total_weight : f64 = particles.iter().map(|p|p.weight).sum();
        
        //(B7) Normalise all the weights together
        #[allow(clippy::assign_op_pattern)]
        particles.iter_mut()
            .for_each(|p| p.weight = p.weight / total_weight );

        Self{
            pop: Population::<P>::new(tolerance, acceptance, particles),
            number:generation_number
        }
    }
}
