use crate::{
    error::{ABCDErr, ABCDResult}, kernel::Kernel, storage::Storage, Generation, Model, Particle
};
use rand::{
    distributions::{Uniform, WeightedIndex},
    prelude::Distribution,
    Rng,
};
use serde::de::DeserializeOwned;
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

    pub fn prepare_kernel_builder<'a, M: Model>(&self, model: &'a M) -> ABCDResult<Option<Cow<'a, M::Kb>>> 
    where 
        M: Model<Parameters = P>,
    {
        match self {
            GenWrapper::Emp(empirical) =>  {
                model
                    .build_kernel_builder(empirical.normalised_particles())
                    .map(|k| Some(k))
                    .map_err(|e|{
                        ABCDErr::InfrastructureError(format!("Something went wrong with kernel builder: {}", e))
                    })
                }
            ,
            GenWrapper::Prior => Ok(None),
        }
    }

    pub fn sample<M, R: Rng>(&self, model: &M, rng: &mut R) -> Cow<P>
    where
        M: Model<Parameters = P>,
        P: Clone,
    {
        match self {
            GenWrapper::Emp(g) => Cow::Borrowed(&g.sample_by_weight(rng).parameters),
            GenWrapper::Prior => Cow::Owned(model.prior_sample(rng)),
        }
    }

    pub fn perturb<M: Model<Parameters = P>>(
        &self,
        parameters: &P,
        model: &M,
        kernel_opt: &Option<Cow<M::K>>,
        rng: &mut impl Rng,
    ) -> ABCDResult<P> 
    where 
        P: Clone {
        match self {
            GenWrapper::Prior => {
                // No perturbation when simple sampling from the prior to build the first generation
                Ok(parameters.clone()) //TODO use a Cow to eliminate the clone
            },
            GenWrapper::Emp(g) => {
                let params = kernel_opt.as_ref()
                    .expect("Kernel is required for perturbing, as this isn't the first gen.")
                    .perturb(parameters, rng)?;
                if model.prior_density(&params) > 0.0 {
                    Ok(params)
                } else {
                    Err(ABCDErr::ParticleErr(
                        "Perturbed particle out of prior bounds.".into(),
                    ))
                }
            },
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
        score: f64,
        tolerance: f64,
        model: &M,
        kernel_opt: &Option<Cow<M::K>>,
    ) -> ABCDResult<Particle<P>>
    where
        M: Model<Parameters = P>,
        P: Debug,
    {
        let result = match self {
            GenWrapper::Emp(g) => 
                g.weigh(
                    parameters, 
                    score, 
                    tolerance, 
                    model, 
                    kernel_opt.as_ref().expect("Kernel is required for weighing, as this isn't the first gen")
                ),
            GenWrapper::Prior => Particle {
                parameters,
                score,
                weight: 1.0,
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

        let weight_dist = WeightedIndex::new(particle_weights).unwrap();
        let uniform_dist = Uniform::from(0..gen.pop.normalised_particles().len());

        Self {
            gen,
            weight_dist,
            uniform_dist,
        }
    }

    pub fn normalised_particles(&self) -> &Vec<Particle<P>>{
        self.gen.pop.normalised_particles()
    }

    pub fn generation_number(&self) -> u16 {
        self.gen.number
    }

    pub fn sample_by_weight<R: Rng>(&self, rng: &mut R) -> &Particle<P>
    where
        P: Clone,
    {
        let sampled_particle_index: usize = self.weight_dist.sample(rng);
        &self.normalised_particles()[sampled_particle_index]
    }

    pub fn sample_uniformly<R: Rng>(&self, rng: &mut R) -> &Particle<P>
    where
        P: Clone,
    {
        let sampled_particle_index: usize = self.uniform_dist.sample(rng);
        &self.normalised_particles()[sampled_particle_index]
    }

    fn weigh<M: Model<Parameters = P>>(
        &self,
        parameters: P,
        score: f64,
        tolerance: f64,
        model: &M,
        kernel: &M::K,
    ) -> Particle<P> {
        // Calculate a **not**-normalised_weight for each particle
        let weight = if score <= tolerance {
            let prior_prob = model.prior_density(&parameters);
            let denominator: f64 = self
                .normalised_particles()
                .iter()
                .map(|prev_gen_particle| {
                    let weight = prev_gen_particle.weight;
                    let pert_density =
                        kernel.pert_density(&prev_gen_particle.parameters, &parameters);
                    weight * pert_density
                })
                .sum();
            prior_prob / denominator
        } else {
            // Notice that the weight may be zero, and no error or warning will occur here.
            // This is because we want a report of acceptance rates across the entire cluster,
            // and the simplest way is to save the rejected particles to storage too.  When
            // subsequently accessing accepted particles, the system won't include the
            // rejected (weight == 0) ones.
            0.0
        };

        Particle {
            parameters,
            score,
            weight,
        }
    }
}
