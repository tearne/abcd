use std::borrow::Cow;

use rand::{Rng, distributions::WeightedIndex, prelude::Distribution};
use statrs::statistics::{Data, Statistics, OrderStatistics};

use crate::{error::{ABCDResult, ABCDErr}, Model, Generation, etc::config::Config, Particle};

pub enum GenWrapper<P>{
    Empirical(Box<Emp<P>>),
    Prior,
}
impl<P> GenWrapper<P> {
    pub fn from_prior() -> Self {
        Self::Prior
    }

    pub fn from_generation(gen: Generation<P>, config: Config) -> Self {
        Self::Empirical(Box::new(Emp::new(gen, config)))
    }

    pub fn generation_number(&self) -> u16 {
        match self {
            GenWrapper::Empirical(g) => g.generation_number(),
            GenWrapper::Prior => 0,
        }
    }

    pub fn sample<M, R: Rng>(&self, model: &M, rng: &mut R) -> Cow<P>
    where 
        M: Model<Parameters = P>,
        P: Clone 
    {
        match self {
            GenWrapper::Empirical(g) => g.sample::<M, R>(rng),
            GenWrapper::Prior => Cow::Owned(model.prior_sample(rng)),
        }
    }

    pub fn perturb<M: Model<Parameters = P>>(&self, parameters: &P, model: &M, rng: &mut impl Rng) -> ABCDResult<P> {
        let params = model.perturb(parameters, rng);        
        if model.prior_density(&params) > 0.0 {
            Ok(params)
        } else {
            Err(ABCDErr::ParticleErr("Perturbed particle out of prior bounds.".into()))
        }
    }
    
    pub fn calculate_tolerance(&self) -> ABCDResult<f64> {
        match self {
            GenWrapper::Empirical(g) => g.calculate_tolerance(),
            GenWrapper::Prior => Ok(f64::MAX),
        }
    }
    pub fn weigh<M: Model<Parameters = P>>(
        &self, 
        parameters: P, 
        scores: Vec<f64>, 
        tolerance: f64, 
        model: &M
    ) -> Particle<P> {
        let fhat = {
            // (B5b) Calculate f^hat by calc'ing proportion less than tolerance
            let number_reps = cast::f64(scores.len());
            let number_reps_less_than_tolerance = scores
                .iter()
                .filter(|score| **score < tolerance)
                .count();
            cast::f64(number_reps_less_than_tolerance)/number_reps
        };
        
        match self {
            GenWrapper::Empirical(g) =>
                g.weigh(parameters, scores, fhat, model),
            GenWrapper::Prior => {
                Particle { 
                    parameters, 
                    scores, 
                    weight:fhat 
                }
            },
        }
    }
}


pub struct Emp<P>{
    gen: Generation<P>,
    dist: WeightedIndex<f64>,
    config: Config
}
impl<P> Emp<P> {
    fn new(gen: Generation<P>, config: Config) -> Self {
        let particle_weights: Vec<f64> = gen
            .pop
            .normalised_particles()
            .iter()
            .map(|p| p.weight)
            .collect();

        let dist = WeightedIndex::new(&particle_weights).unwrap();
        
        Self { 
            gen, 
            dist,
            config 
        }
    }

    fn generation_number(&self) -> u16 {
        self.gen.number
    }

    fn sample<M, R: Rng>(&self, rng: &mut R) -> Cow<P>
    where 
        M: Model<Parameters = P>,
        P: Clone
     {
        let sampled_particle_index: usize = self.dist.sample(rng);
        let particles = &self
            .gen
            .pop
            .normalised_particles()[sampled_particle_index];
        let params = &particles.parameters;
        Cow::Borrowed(params)
    }

    fn calculate_tolerance(&self) -> ABCDResult<f64> {
        // Get distribution of scores from last generation then reduce by tolerance descent rate (configured) - crate exists for percentile => 
        let score_distribution: ABCDResult<Vec<f64>> = self.gen
            .pop
            .normalised_particles()
            .iter()
            .map(|particle| {
                let mean_scores: f64 = particle.scores.clone().mean();
                match mean_scores.is_nan() {
                    false => Ok(mean_scores),
                    true => Err(ABCDErr::SystemError("Mean score is not a number.".into()))
                }
            })
            .collect();

        let mut score_distribution = Data::new(score_distribution?);
        let new_tolerance = score_distribution.percentile(self.config.algorithm.tolerance_descent_percentile);

        match new_tolerance.is_nan() {
            false => {
                log::info!("Tolerance calculated as {new_tolerance}");
                Ok(new_tolerance)
            },
            true => Err(ABCDErr::InfrastructureError("Tolerance is not a number.".into()))//TODO rename SystemError?
        }
    }

    fn weigh<M: Model<Parameters = P>>(&self, parameters: P, scores: Vec<f64>, fhat: f64, model: &M) -> Particle<P> {
        // (B6) Calculate not_normalised_weight for each particle from its f^hat (f^hat(p) * prior(p)) / denom)
        let prior_prob = model.prior_density(&parameters);
        let denominator : f64 = self.gen.pop.normalised_particles()
                .iter()
                .map(|prev_gen_particle| {
                    let weight = prev_gen_particle.weight;
                    let pert_density = model.pert_density(&prev_gen_particle.parameters, &parameters);
                    weight * pert_density
                }).sum();
        let weight = fhat*prior_prob / denominator;
        Particle { 
            parameters, 
            scores, 
            weight 
        }
    }
}