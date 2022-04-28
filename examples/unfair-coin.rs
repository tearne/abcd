use std::{ops::Range, path::Path};

use abcd::{Model, error::ABCDResult, etc::config::Config, storage::config::StorageConfig};
use path_absolutize::Absolutize;
use serde::{Deserialize, Serialize};
use rand::{Rng, prelude::ThreadRng};
use statrs::distribution::Normal;
use anyhow::Result;

#[derive(Serialize, Deserialize, Debug,Clone)]
struct MyParameters {
    heads: f64,
}

#[derive(Debug)]
struct UniformParams {
    range: Range<f64>,
    normal: Normal,
}
impl UniformParams {
    fn new(lower: f64, upper: f64) -> Self {
        assert!(lower < upper);
        Self{
            range: Range{start: lower, end: upper},
            normal: Normal::new(0.0,0.2).unwrap()
        }
    }
    
    fn prior_sample(&self, random: &mut ThreadRng) -> f64 {
        random.gen_range(self.range.clone())
    }

    fn prior_density(&self, v: f64) -> f64 {
        let low = self.range.start;
        let high = self.range.end;
        if v > high || v < low { 0.0 }
        else { 1.0 }
    }

    fn kernel_sample(&self, random: &mut ThreadRng) -> f64 {
        random.sample(self.normal)
    }

    fn kernel_density(&self, v: f64) -> f64 {
        use statrs::distribution::Continuous;
        self.normal.pdf(v)
    }
}

#[derive(Debug)]
struct MyModel {
    heads_range: UniformParams,
}

impl MyModel {
    pub fn new() -> Self {
        MyModel { 
            heads_range: UniformParams::new(0.0,1.0),
        }
    }
}

impl Model for MyModel {
    type Parameters = MyParameters;

    fn prior_sample(&self, random: &mut ThreadRng) -> Self::Parameters {
        let heads: f64 = self.heads_range.prior_sample(random);
        MyParameters {
            heads
        }
    }

    fn prior_density(&self, p: &Self::Parameters) -> f64 {
        let density : f64 = {
        self.heads_range.prior_density(p.heads)};
        density
    }

    fn perturb(&self, _p: &Self::Parameters, random: &mut ThreadRng) -> Self::Parameters {
        let heads: f64 = _p.heads + self.heads_range.kernel_sample(random);
        MyParameters {
            heads
        }
    }

    fn pert_density(&self, _from: &Self::Parameters, _to: &Self::Parameters) -> f64 {
        let pert_density : f64 = {
            self.heads_range.kernel_density(_from.heads - _to.heads) };
            pert_density
    }

    fn score(&self, p: &Self::Parameters) -> ABCDResult<f64> {
        let mut random = rand::thread_rng();
        let mut heads_count:f64 = 0.0;

        for numTrials in 1..100 {
            let coin_toss = random.gen_bool(p.heads);
            if coin_toss  {
                heads_count = heads_count + 1.0;
            }
        }

        let simulated_heads = heads_count / 100.0;
        if simulated_heads == 0.7 { Ok(0.0) }
        else { Ok(1.0) }
    }
}

fn main() -> Result<()> {
    env_logger::init();

    let m = MyModel::new();
    let mut random = rand::thread_rng();

    let path = Path::new("./config.toml"); 
    println!("Config path {:?}", path.absolutize().unwrap());
    let config = Config::from_path(path);

    log::info!("You are running with config\n {:#?}", &config);

    let storage = config.storage.build_s3()?;

    abcd::run(m, config, storage, &mut random)?;
    //println!("{:?}", &m);
    Ok(())
}
