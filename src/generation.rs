use std::borrow::Cow;

use rand::prelude::ThreadRng;
use statrs::statistics::{Data, Statistics, OrderStatistics};

use crate::{error::{ABCDResult, ABCDError}, Model, Generation, etc::config::Config, Particle};

pub trait GenerationOps<P> {
    fn sample<M>(&self, model: &M, random: &mut ThreadRng) -> ABCDResult<Cow<P>> 
    where 
        M: Model<Parameters = P>,
        P: Clone;

    fn perturb<M: Model<Parameters = P>>(&self, parameters: &P, model: &M, random: &mut ThreadRng) -> ABCDResult<P> {
        let params = model.perturb(parameters,random);        
        if model.prior_density(&params) > 0.0 {
            Ok(params)
        } else {
            Err(ABCDError::AlgortihmError("Proposed particle out of prior bounds.".into()))
        }
    }
    
    fn calculate_tolerance(&self) -> ABCDResult<f64>;
    fn weigh<M: Model<Parameters = P>>(&self, params: P, scores: Vec<f64>, tolerance: f64, model: &M) -> Particle<P>;

    fn calculate_fhat(scores: &[f64], tolerance: f64) -> f64 {
        // (B5b) Calculate f^hat by calc'ing proportion less than tolerance
        let number_reps = cast::f64(scores.len());
        let number_reps_less_than_tolerance = scores
            .iter()
            .filter(|score| **score < tolerance)
            .count();
        cast::f64(number_reps_less_than_tolerance)/number_reps
    }
}
pub struct EmpiricalGeneration<P>{
    gen: Generation<P>,
    config: Config
}
impl<P> EmpiricalGeneration<P> {
    pub fn new(gen: Generation<P>, config: Config) -> Self {
        Self { gen, config }
    }
}
impl<P> GenerationOps<P> for EmpiricalGeneration<P> {
    fn sample<M>(&self, _model: &M, random: &mut ThreadRng) -> ABCDResult<Cow<P>>
    where 
        M: Model<Parameters = P>,
        P: Clone
     {
        self.gen.sample(random)
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
                    true => Err(ABCDError::AlgortihmError("Mean score is not a number.".into()))
                }
            })
            .collect();

        let mut score_distribution = Data::new(score_distribution?);
        let new_tolerance = score_distribution.percentile(self.config.algorithm.tolerance_descent_percentile);

        match new_tolerance.is_nan() {
            false => Ok(new_tolerance),
            true => Err(ABCDError::AlgortihmError("Tolerance is not a number.".into()))
        }
    }

    fn weigh<M: Model<Parameters = P>>(&self, parameters: P, scores: Vec<f64>, tolerance: f64, model: &M) -> Particle<P> {
        // (B6) Calculate not_normalised_weight for each particle from its f^hat (f^hat(p) * prior(p)) / denom)
        let fhat = Self::calculate_fhat(&scores, tolerance);
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
pub struct PriorGeneration{}
impl<P> GenerationOps<P> for PriorGeneration {
    fn sample<M> (&self, model: &M, random: &mut ThreadRng) -> ABCDResult<Cow<P>> 
    where 
        M: Model<Parameters = P>,
        P: Clone,
    {
        Ok(Cow::Owned(model.prior_sample(random)))
    }

    fn calculate_tolerance(&self) -> ABCDResult<f64> {
        Ok(f64::MAX)
    }


    fn weigh<M: Model<Parameters = P>>(&self, parameters: P, scores: Vec<f64>, tolerance: f64, _model: &M) -> Particle<P> {
        let fhat = <Self as GenerationOps<P>>::calculate_fhat(&scores, tolerance);
        Particle { 
            parameters, 
            scores, 
            weight:fhat 
        }
    }
}