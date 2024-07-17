use abcd::{config::Config, error::VectorConversionError, kernel::olcm::{OLCMKernel, OLCMKernelBuilder}, types::Vector, wrapper::GenWrapper, Model, ABCD};
use color_eyre::eyre;
use nalgebra::{DVector, SMatrix, Vector2};
use path_absolutize::Absolutize;
use rand::Rng;
use serde::{Deserialize, Serialize};
use statrs::distribution::Normal;
use std::{error::Error, ops::Range, path::Path};
use tokio::runtime::Runtime;

/// We have two coins, A & B, neither of which need be fair.
/// Let P(Heads_A)=alpha and P(Head_B)=beta.
/// 
/// An experiment involves tossing the pair and applying an
/// XOR gate to the results, so that the overall result is 
/// positive if and only if precisely one of them is heads.
/// 
/// We toss the pair 100 times and count the number of 
/// positive results as 38.  What is the posterior 
/// distribution of alpha and beta?

#[derive(Serialize, Deserialize, Debug, Clone)]
struct MyParameters {
    heads_a: f64,
    heads_b: f64,
}

impl Vector<2> for MyParameters {
    fn to_column_vector(&self) -> SMatrix<f64, 2, 1> {
        Vector2::new(self.heads_a, self.heads_b)
    }

    fn from_column_vector(v: DVector<f64>) -> Result<Self, abcd::error::VectorConversionError> {
        let values = v.iter().cloned().collect::<Vec<f64>>();
            if values.len() != 2 {
                return Err(VectorConversionError(format!(
                    "Wrong number of arguments.  Expected 2, got {}",
                    values.len()
                )));
            } else {
                Ok(MyParameters {
                    heads_a: values[0],
                    heads_b: values[1],
                })
            }
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

#[derive(Debug)]
struct Kernel {
    normal: Normal,
}
impl Kernel {
    fn new(std_dev: f64) -> Self {
        Self {
            normal: Normal::new(0.0, std_dev).unwrap(),
        }
    }

    fn sample(&self, rng: &mut impl Rng) -> f64 {
        rng.sample(self.normal)
    }

    fn density(&self, v: f64) -> f64 {
        use statrs::distribution::Continuous;
        self.normal.pdf(v)
    }
}

#[derive(Debug)]
struct MyModel {
    prior: Uniform,
    observed: f64,
    num_trials: u64,
}

impl MyModel {
    pub fn new(observed_proportion_positive: f64, num_trials: u64) -> Self {
        MyModel {
            prior: Uniform::new(0.0, 1.0),
            observed: observed_proportion_positive,
            num_trials,
        }
    }
}

impl Model for MyModel {
    type Parameters = MyParameters;
    type K = OLCMKernel<2, MyParameters>; 
    type Kb = OLCMKernelBuilder<2, MyParameters>;

    fn prior_sample(&self, rng: &mut impl Rng) -> Self::Parameters {
        let heads: f64 = self.prior.sample(rng);
        MyParameters { 
            heads_a : self.prior.sample(rng),
            heads_b : self.prior.sample(rng),
        }
    }

    fn prior_density(&self, p: &Self::Parameters) -> f64 {
        let density: f64 = { self.prior.density(p.heads) };
        density
    }
    
    fn build_kernel_builder_for_generation(&self, prev_gen: &GenWrapper<Self::Parameters>) -> Result<&Self::Kb, Box<dyn Error>> {
        OLCMKernelBuilder::new(prev_gen.pa)
    }

    // fn perturb(&self, _p: &Self::Parameters, rng: &mut impl Rng) -> Self::Parameters {
    //     let heads: f64 = _p.heads + self.kernel.sample(rng);
    //     MyParameters { heads }
    // }

    // fn pert_density(&self, _from: &Self::Parameters, _to: &Self::Parameters) -> f64 {
    //     let pert_density: f64 = { self.kernel.density(_from.heads - _to.heads) };
    //     pert_density
    // }

    fn score(&self, p: &Self::Parameters) -> Result<f64, Self::E> {
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
