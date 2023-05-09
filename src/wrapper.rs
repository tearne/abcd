use crate::{
    config::Config,
    error::{ABCDErr, ABCDResult},
    storage::Storage,
    Generation, Model, Particle,
};
use rand::{distributions::{WeightedIndex, Uniform}, prelude::Distribution, Rng};
use serde::de::DeserializeOwned;
use statrs::statistics::{Data, OrderStatistics, Statistics};
use std::{borrow::Cow, fmt::Debug};

pub enum GenWrapper<P> {
    Emp(Box<Empirical<P>>),
    Prior,
}
impl<P> GenWrapper<P> {
    pub fn from_prior() -> Self {
        Self::Prior
    }

    pub fn from_generation(gen: Generation<P>) -> Self {
        Self::Emp(Box::new(Empirical::new(gen)))
    }

    pub fn generation_number(&self) -> u16 {
        match self {
            GenWrapper::Emp(g) => g.generation_number(),
            GenWrapper::Prior => 0,
        }
    }

    pub fn load_previous_gen<M: Model, S: Storage>(
        storage: &S,
    ) -> ABCDResult<GenWrapper<M::Parameters>>
    where
        M: Model<Parameters = P>,
        P: DeserializeOwned + Debug,
    {
        if storage.previous_gen_number()? == 0 {
            Ok(GenWrapper::from_prior())
        } else {
            Ok(GenWrapper::from_generation(storage.load_previous_gen()?))
        }
    }

    pub fn sample<M, R: Rng>(&self, model: &M, rng: &mut R) -> Cow<P>
    where
        M: Model<Parameters = P>,
        P: Clone,
    {
        match self {
            GenWrapper::Emp(g) => g.sample_by_weight::<R>(rng),
            GenWrapper::Prior => Cow::Owned(model.prior_sample(rng)),
        }
    }

    pub fn perturb<M: Model<Parameters = P>>(
        &self,
        parameters: &P,
        model: &M,
        rng: &mut impl Rng,
    ) -> ABCDResult<P> {
        let params = model.perturb(parameters, rng);
        if model.prior_density(&params) > 0.0 {
            Ok(params)
        } else {
            Err(ABCDErr::ParticleErr(
                "Perturbed particle out of prior bounds.".into(),
            ))
        }
    }

    pub fn next_gen_tolerance(&self) -> ABCDResult<f64> {
        match self {
            GenWrapper::Emp(g) => Ok(g.gen.next_gen_tolerance),
            GenWrapper::Prior => Ok(f64::MAX),
        }
    }

    pub fn weigh<M>(
        &self,
        parameters: P,
        scores: Vec<f64>,
        tolerance: f64,
        model: &M,
    ) -> ABCDResult<Particle<P>>
    where
        M: Model<Parameters = P>,
        P: Debug,
    {
        let fhat = {
            // (B5b) Calculate f^hat by calc'ing proportion less than tolerance
            let number_reps = cast::f64(scores.len());
            let number_reps_less_than_tolerance =
                scores.iter().filter(|score| **score < tolerance).count();
            cast::f64(number_reps_less_than_tolerance) / number_reps
        };

        log::debug!("fhat {} from scores {:?}", fhat, &scores);

        let result = match self {
            GenWrapper::Emp(g) => g.weigh(parameters, scores, fhat, model),
            GenWrapper::Prior => Particle {
                parameters,
                scores,
                weight: fhat,
            },
        };

        Ok(result)
    }
}

pub struct Empirical<P> {
    gen: Generation<P>,
    weight_dist: WeightedIndex<f64>,
    uniform_dist: Uniform<usize>,
}
impl<P> Empirical<P> {
    pub fn new(gen: Generation<P>) -> Self {
        let particle_weights: Vec<f64> = gen
            .pop
            .normalised_particles()
            .iter()
            .map(|p| p.weight)
            .collect();

        let weight_dist = WeightedIndex::new(&particle_weights).unwrap();
        let uniform_dist = Uniform::from(0..gen.pop.normalised_particles().len());

        Self { 
            gen, 
            weight_dist,
            uniform_dist, 
        }
    }

    pub fn generation_number(&self) -> u16 {
        self.gen.number
    }

    pub fn sample_by_weight<R: Rng>(&self, rng: &mut R) -> Cow<P>
    where
        P: Clone,
    {
        let sampled_particle_index: usize = self.weight_dist.sample(rng);
        let particle= &self.gen.pop.normalised_particles()[sampled_particle_index];
        let params = &particle.parameters;
        Cow::Borrowed(params)
    }

    pub fn sample_uniformly<R: Rng>(&self, rng: &mut R) -> Cow<Particle<P>>
    where
        P: Clone,
    {
        let sampled_particle_index: usize = self.uniform_dist.sample(rng);
        let particle = &self.gen.pop.normalised_particles()[sampled_particle_index];
        //let params = &particle.parameters;
        Cow::Borrowed(particle)
    }

    fn weigh<M: Model<Parameters = P>>(
        &self,
        parameters: P,
        scores: Vec<f64>,
        fhat: f64,
        model: &M,
    ) -> Particle<P> {
        // (B6) Calculate not_normalised_weight for each particle from its f^hat (f^hat(p) * prior(p)) / denom)
        let prior_prob = model.prior_density(&parameters);
        let denominator: f64 = self
            .gen
            .pop
            .normalised_particles()
            .iter()
            .map(|prev_gen_particle| {
                let weight = prev_gen_particle.weight;
                let pert_density = model.pert_density(&prev_gen_particle.parameters, &parameters);
                weight * pert_density
            })
            .sum();
        let weight = fhat * prior_prob / denominator;
        Particle {
            parameters,
            scores,
            weight,
        }
    }
}
