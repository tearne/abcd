use abcd::{config::Config, error::ABCDErr, kernel::olcm::{OLCMKernel, OLCMKernelBuilder}, wrapper::GenWrapper, Model, ABCD};
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
/// Our experiment involves tossing both coins and applying an
/// XOR to the results, so that the overall result is positive
/// if and only if precisely one coin is heads.
/// 
/// We toss the pair 100 times and count the number of 
/// positive results as 38.  Given no prior knowledge of alpha
/// and beta (uniform prior), what is their (two dimensional) 
/// posterior distribution?
/// 
/// In the ABCD implementation, we are going to use an OLCM 
/// kernel implementation, which is provided by the ABCD crate.

#[derive(Serialize, Deserialize, Debug, derive_more::Add, derive_more::Sub, Copy, Clone)]
struct ProbabilityHeads {
    alpha: f64,
    beta: f64,
}

impl TryFrom<DVector<f64>> for ProbabilityHeads {
    type Error = ABCDErr;
    
    fn try_from(value: DVector<f64>) -> Result<Self, Self::Error> {
        if value.len() != 2 {
            return Err(ABCDErr::VectorConversionError(format!(
                "Wrong number of arguments.  Expected 2, got {}",
                value.len()
            )));
        } else {
            Ok(ProbabilityHeads {
                alpha: value[0],
                beta: value[1],
            })
        }
    }
}

impl Into<DVector<f64>> for ProbabilityHeads {
    fn into(self) -> DVector<f64> {
        DVector::from_column_slice(&[self.alpha, self.beta])
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
    type Parameters = ProbabilityHeads;
    type K = OLCMKernel<ProbabilityHeads>; 
    type Kb = OLCMKernelBuilder<ProbabilityHeads>;

    fn prior_sample(&self, rng: &mut impl Rng) -> Self::Parameters {
        let heads: f64 = self.prior.sample(rng);
        ProbabilityHeads { 
            alpha : self.prior.sample(rng),
            beta : self.prior.sample(rng),
        }
    }

    fn prior_density(&self, p: &Self::Parameters) -> f64 {
        let density: f64 = { self.prior.density(p.alpha) * self.prior.density(p.beta)};
        density
    }
    
    fn build_kernel_builder_for_generation(&self, prev_gen: &GenWrapper<Self::Parameters>) -> Result<&Self::Kb, Box<dyn Error>> {
        OLCMKernelBuilder::new(prev_gen.)
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
