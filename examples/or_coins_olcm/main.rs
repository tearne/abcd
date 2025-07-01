use abcd::{
    config::AbcdConfig, error::ABCDErr, kernel::olcm::{OLCMKernel, OLCMKernelBuilder}, storage::{config::StorageConfig, s3::S3System}, Model, Particle, ABCD
};
use color_eyre::eyre;
use nalgebra::DVector;
use path_absolutize::Absolutize;
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::{borrow::Cow, error::Error, ops::Range, path::Path};
use tokio::runtime::Runtime;

/// We have two coins, A & B, neither of which need be fair.
/// Let P(Heads_A)=alpha and P(Head_B)=beta.
///
/// Our experiment involves tossing both coins and applying an
/// OR to the results, so that the overall result is positive
/// if either coin is heads.
///
/// We toss the pair 100 times and count the number of
/// positive results as 75.  Given no prior knowledge of alpha
/// and beta (uniform prior), what is their (two dimensional)
/// posterior distribution?
///
/// We use an OLCM kernel provided by the ABCD crate.

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

impl From<ProbabilityHeads> for DVector<f64> {
    fn from(value: ProbabilityHeads) -> Self {
        DVector::from_column_slice(&[value.alpha, value.beta])
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
struct MyModel {
    prior: Uniform,
    observed_count: u8,
    num_trials: u64,
}

impl MyModel {
    pub fn new(observed_count: u8, num_trials: u64) -> Self {
        MyModel {
            prior: Uniform::new(0.0, 1.0),
            observed_count,
            num_trials,
        }
    }
}

impl Model for MyModel {
    type Parameters = ProbabilityHeads;
    type K = OLCMKernel<ProbabilityHeads>;
    type Kb = OLCMKernelBuilder<ProbabilityHeads>;

    fn prior_sample(&self, rng: &mut impl Rng) -> Self::Parameters {
        ProbabilityHeads {
            alpha: self.prior.sample(rng),
            beta: self.prior.sample(rng),
        }
    }

    fn prior_density(&self, p: &Self::Parameters) -> f64 {
        let density: f64 = { self.prior.density(p.alpha) * self.prior.density(p.beta) };
        density
    }

    fn build_kernel_builder<'a>(
        &'a self,
        prev_gen_particles: &Vec<Particle<Self::Parameters>>,
    ) -> Result<Cow<'a, Self::Kb>, Box<dyn Error>> {
        OLCMKernelBuilder::new(prev_gen_particles)
            .map(|k| Cow::Owned(k))
            .map_err(|e| e.into())
    }

    fn score(&self, p: &Self::Parameters) -> Result<f64, Box<dyn Error>> {
        let mut random = rand::thread_rng();
        let mut simulated_count: u8 = 0;

        for _ in 0..self.num_trials {
            let combined_result = random.gen_bool(p.alpha) || random.gen_bool(p.beta);
            if combined_result {
                simulated_count += 1;
            }
        }

        let diff = (self.observed_count as i64 - simulated_count as i64).abs() as f64;
        eyre::Result::Ok(diff)
    }
}

fn main() -> eyre::Result<()> {
    env_logger::init();

    let observed_count = 75u8;
    let num_trials = 100;

    let m = MyModel::new(observed_count, num_trials);
    let mut random = rand::thread_rng();

    let path = Path::new("./config.toml").absolutize().unwrap();
    log::info!("Load config from {:?}", path);
    let config = {
        let str = std::fs::read_to_string(path)?;
        toml::from_str::<AbcdConfig>(&str).expect("Expected parsed config")
    };

    let runtime = Runtime::new().unwrap();
    let storage = S3System::new(
        "$TEST_BUCKET",
        "$TEST_PREFIX",
        runtime.handle().clone()
    )?;

    ABCD::run(m, config, storage, &mut random)?;

    Ok(())
}
