use rand::Rng;
use rand::prelude::*;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::fmt::Debug;
use std::borrow::Cow;
use rand::distributions::WeightedIndex;

use crate::error::ABCDResult;

pub trait Model {
    type Parameters: Serialize + DeserializeOwned + Debug + Clone;

    fn prior_sample(&self, random: &mut ThreadRng) -> Self::Parameters; //TODO check density of sampled value is NOT 0
    fn prior_density(&self, p: &Self::Parameters) -> f64;

    fn perturb(&self, p: &Self::Parameters,random: &mut ThreadRng) -> Self::Parameters;
    fn pert_density(&self, from: &Self::Parameters, to: &Self::Parameters) -> f64;

    fn score(&self, p: &Self::Parameters) -> ABCDResult<f64>;
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
    acceptance: f64,
    normalised_particles: Vec<Particle<P>>,
}
impl<P> Population<P> {
    pub fn new(tolerance: f64, acceptance: f64, normalised_particles: Vec<Particle<P>>) -> Self {
        // let totalweight = normalised_particles.iter().map(|p|p.weight).sum::<f64>();
        // println!("TOTAL WEIGHT is {}",totalweight);
        // let check = (normalised_particles.iter().map(|p|p.weight).sum::<f64>() - 1.0).abs();
        // println!("REMAINDER is {}",check);
        // println!("EPSILON is {}",f64::EPSILON);
        // assert!((normalised_particles.iter().map(|p|p.weight).sum::<f64>() - 1.0).abs() < f64::EPSILON);

        Self {
            tolerance,
            acceptance,
            normalised_particles
        }
    }

    pub fn tolerance(&self) -> f64 {
        self.tolerance
    }

    pub fn acceptance(&self) -> f64 {
        self.acceptance
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
        acceptance: f64 //TODO change to an f16?
    ) -> Self{
        let total_weight : f64 = particles.iter().map(|p|p.weight).sum();
        
        //(B7) Normalise all the weights together
        #[allow(clippy::assign_op_pattern)]
        particles.iter_mut()
            .for_each(|p| p.weight = p.weight / total_weight );

        Self{
            pop: Population::<P>::new(tolerance,acceptance,particles),
            number:generation_number
        }
    }
    
    pub fn sample(&self, random: &mut ThreadRng) -> Cow<P>
    where 
        P: Clone
     {
        // TODO can't we pre-calculate the weights table to avoid rebuilding on every proposal?
        let particle_weights: Vec<f64> = self
            .pop
            .normalised_particles()
            .iter()
            .map(|p| p.weight)
            .collect();

        let dist = WeightedIndex::new(&particle_weights).unwrap();
        let sampled_particle_index: usize = dist.sample(random);
        let particles = &self
            .pop
            .normalised_particles()[sampled_particle_index];
        let params = &particles.parameters;
        Cow::Borrowed(params)
    }
}
