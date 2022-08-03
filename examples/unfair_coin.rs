use std::{ops::Range, path::Path};

use abcd::{Model, error::ABCDResult, etc::config::Config};
use path_absolutize::Absolutize;
use serde::{Deserialize, Serialize};
use rand::{Rng, prelude::ThreadRng};
use statrs::distribution::Normal;
use anyhow::Result;
use tokio::runtime::Runtime;

#[derive(Serialize, Deserialize, Debug,Clone)]
struct MyParameters {
    heads: f64,
}

#[derive(Debug)]
struct Uniform {
    range: Range<f64>,
    
}
impl Uniform {
    fn new(lower: f64, upper: f64) -> Self {
        assert!(lower < upper);
        Self{
            range: Range{start: lower, end: upper},
        }
    }
    
    fn sample(&self, random: &mut ThreadRng) -> f64 {
        random.gen_range(self.range.clone())
    }

    fn density(&self, v: f64) -> f64 {
        let low = self.range.start;
        let high = self.range.end;
        if v > high || v < low { 0.0 }
        else { 1.0 }
    }
}

#[derive(Debug)]
struct Kernel {
    normal: Normal,
}
impl Kernel {
    fn new(std_dev: f64) -> Self {
        Self{ 
            normal: Normal::new(0.0,std_dev).unwrap()
        }
    }

    fn sample(&self, random: &mut ThreadRng) -> f64 {
        random.sample(self.normal)
    }

    fn density(&self, v: f64) -> f64 {
        use statrs::distribution::Continuous;
        self.normal.pdf(v)
    }
}

#[derive(Debug)]
struct MyModel {
    prior: Uniform,
    kernel: Kernel,
    observed: f64,
    reps: u64,
}

impl MyModel {
    pub fn new(observed_proportion_heads: f64, reps: u64) -> Self {
        MyModel { 
            prior: Uniform::new(0.0,1.0),
            kernel: Kernel::new(0.05),
            observed: observed_proportion_heads,
            reps,
        }
    }
}

impl Model for MyModel {
    type Parameters = MyParameters;

    fn prior_sample(&self, random: &mut ThreadRng) -> Self::Parameters {
        let heads: f64 = self.prior.sample(random);
        MyParameters {
            heads
        }
    }

    fn prior_density(&self, p: &Self::Parameters) -> f64 {
        let density : f64 = {
        self.prior.density(p.heads)};
        density
    }

    fn perturb(&self, _p: &Self::Parameters, random: &mut ThreadRng) -> Self::Parameters {
        let heads: f64 = _p.heads + self.kernel.sample(random);
        MyParameters {
            heads
        }
    }

    fn pert_density(&self, _from: &Self::Parameters, _to: &Self::Parameters) -> f64 {
        let pert_density : f64 = {
            self.kernel.density(_from.heads - _to.heads) };
            pert_density
    }

    fn score(&self, p: &Self::Parameters) -> ABCDResult<f64> {
        let mut random = rand::thread_rng();
        let mut heads_count: u64 = 0;

        for _ in 0..self.reps {
            let coin_toss = random.gen_bool(p.heads);
            if coin_toss  {
                heads_count += 1;
            }
        }

        let simulated = heads_count as f64 / self.reps as f64;
        let diff = (self.observed - simulated).abs();
        Ok(diff)
    }
}

fn main() -> Result<()> {
    env_logger::init();

    let observed_proportion_heads = 0.7;
    let reps = 100;

    let m = MyModel::new(observed_proportion_heads, reps);
    let mut random = rand::thread_rng();

    let path = Path::new("./config.toml").absolutize().unwrap(); 
    log::info!("Load config from {:?}", path);
    let config = Config::from_path(path);

    let runtime = Runtime::new().unwrap();
    let handle = runtime.handle();

    let storage = config.storage.build_s3(handle.clone())?;

    abcd::run(m, config, storage, &mut random)?;

    Ok(())
}
