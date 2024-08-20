use abcd::{config::Config, error::{ABCDErr, ABCDResult}, kernel::{Kernel, TrivialKernel}, wrapper::GenWrapper, Model, ABCD};
use color_eyre::eyre;
use nalgebra::DVector;
use path_absolutize::Absolutize;
use rand::Rng;
use serde::{Deserialize, Serialize};
use statrs::distribution::Normal;
use std::{error::Error, marker::PhantomData, ops::Range, path::Path};
use tokio::runtime::Runtime;

#[derive(Serialize, Deserialize, derive_more::Add, derive_more::Sub, Debug, Clone)]
struct MyParameters {
    heads: f64,
}
impl TryFrom<DVector<f64>> for MyParameters {
    type Error = ABCDErr;

    fn try_from(value: DVector<f64>) -> Result<Self, Self::Error> {
        todo!()
    }
}
impl Into<DVector<f64>> for MyParameters {
    fn into(self) -> DVector<f64> {
        todo!()
    }
}

#[derive(Debug)]
struct Uniform {
    range: Range<f64>,
}
impl Uniform {
    fn new(lower: f64, upper: f64) -> Self {
        assert!(lower < upper);
        Self {
            range: Range {
                start: lower,
                end: upper,
            },
        }
    }

    fn sample(&self, rng: &mut impl Rng) -> f64 {
        rng.gen_range(self.range.clone())
    }

    fn density(&self, v: f64) -> f64 {
        let low = self.range.start;
        let high = self.range.end;
        if v > high || v < low {
            0.0
        } else {
            1.0
        }
    }
}

#[derive(Debug, Clone)]
struct NormalKernel {
    normal: Normal,
}
impl NormalKernel {
    fn new(std_dev: f64) -> Self {
        Self {
            normal: Normal::new(0.0, std_dev).unwrap(),
        }
    }
}
impl Kernel<MyParameters> for NormalKernel {
    fn perturb(&self, p: &MyParameters, rng: &mut impl Rng) -> ABCDResult<MyParameters> {
        let heads: f64 = p.heads + rng.sample(self.normal);
        Ok(MyParameters { heads })
    }

    fn pert_density(&self, from: &MyParameters, to: &MyParameters) -> f64 {
        let pert_density: f64 = { 
            let diff = from.heads - to.heads;
            use statrs::distribution::Continuous;
            self.normal.pdf(diff)
        };
        pert_density
    }
}

struct MyModel {
    prior: Uniform,
    kernel: TrivialKernel<MyParameters, NormalKernel>,
    observed: f64,
    num_trials: u64,
    phantom: PhantomData<MyParameters>
}

impl MyModel {
    pub fn new(observed_proportion_heads: f64, num_trials: u64) -> Self {
        MyModel {
            prior: Uniform::new(0.0, 1.0),
            kernel: TrivialKernel::from(NormalKernel::new(0.1)),
            observed: observed_proportion_heads,
            num_trials,
            phantom: PhantomData::<MyParameters>::default(),
        }
    }
}


impl Model for MyModel {
    type Parameters = MyParameters;
    type K = TrivialKernel<MyParameters, NormalKernel>;
    type Kb = Self::K;

    fn prior_sample(&self, rng: &mut impl Rng) -> Self::Parameters {
        let heads: f64 = self.prior.sample(rng);
        MyParameters { heads }
    }

    fn prior_density(&self, p: &Self::Parameters) -> f64 {
        let density: f64 = { self.prior.density(p.heads) };
        density
    }

    fn build_kernel_builder_for_generation(&self, _prev_gen: &GenWrapper<Self::Parameters>) -> Result<&Self::Kb, Box<dyn Error>> {
        Ok(&self.kernel)
    }

    fn score(&self, p: &Self::Parameters) -> Result<f64, Box<dyn Error>> {
        let mut random = rand::thread_rng();
        let mut heads_count: u64 = 0;

        for _ in 0..self.num_trials {
            let coin_toss = random.gen_bool(p.heads);
            if coin_toss {
                heads_count += 1;
            }
        }

        let simulated = heads_count as f64 / self.num_trials as f64;
        let diff = (self.observed - simulated).abs();
        eyre::Result::Ok(diff)
    }
}

fn main() -> eyre::Result<()> {
    env_logger::init();

    let observed_proportion_heads = 0.8;
    let num_trials = 50;

    let m = MyModel::new(observed_proportion_heads, num_trials);
    let mut random = rand::thread_rng();

    let path = Path::new("./config.toml").absolutize().unwrap();
    println!("---{:?}", path);
    log::info!("Load config from {:?}", path);
    let config = Config::from_path(path)?;
    println!("+++{:?}", &config);
    // exit(1);

    let runtime = Runtime::new().unwrap();
    let handle = runtime.handle();

    let storage = config.storage.build_s3(handle.clone())?;

    ABCD::run(m, config, storage, &mut random)?;

    Ok(())
}
