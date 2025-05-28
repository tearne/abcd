use std::{
    ffi::OsStr,
    fs, io,
    path::{Path, PathBuf},
};

use abcd::{
    error::{ABCDErr, ABCDResult},
    kernel::{olcm::OLCMKernelBuilder, KernelBuilder},
    Generation, Particle,
};
use nalgebra::DVector;
use rand::{rngs::SmallRng, SeedableRng};
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Serialize, Deserialize, Debug, Copy, Clone, derive_more::Add, derive_more::Sub)]
struct TestParams {
    alpha: f64,
    beta: f64,
}
impl TestParams {
    pub fn from_slice(slice: &[f64]) -> Self {
        Self {
            alpha: slice[0],
            beta: slice[1],
        }
    }
}

impl TryFrom<DVector<f64>> for TestParams {
    type Error = ABCDErr;

    fn try_from(value: DVector<f64>) -> Result<Self, Self::Error> {
        if value.len() != 2 {
            return Err(ABCDErr::VectorConversionError(format!(
                "Wrong number of arguments.  Expected 2, got {}",
                value.len()
            )));
        } else {
            Ok(TestParams {
                alpha: value[0],
                beta: value[1],
            })
        }
    }
}
impl Into<DVector<f64>> for TestParams {
    fn into(self) -> DVector<f64> {
        DVector::from_column_slice(&[self.alpha, self.beta])
    }
}

pub fn main() -> ABCDResult<()> {
    // Find the biggest available generation to use to generate samples
    fn available_generations() -> io::Result<Vec<PathBuf>> {
        let mut generations = vec![];

        for path in fs::read_dir("../../out/xor_coins/")? {
            let path = path?.path();
            if let Some("json") = path.extension().and_then(OsStr::to_str) {
                generations.push(path.to_owned());
            }
        }
        Ok(generations)
    }

    let generations = available_generations().unwrap();
    let mut gen_names: Vec<_> = generations
        .iter()
        .map(|gen| gen.display().to_string())
        .collect();
    gen_names.sort();
    let biggest_gen = gen_names.last().unwrap();

    println!("Using generation: {:?} to generate samples", biggest_gen);

    let path = biggest_gen;
    let generation: Generation<TestParams> =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    let population = generation.pop;

    let alpha_mean = &population
        .normalised_particles()
        .iter()
        .enumerate()
        .min_by_key(|(_, p)| ((p.parameters.alpha - 0.5).abs() * 100000.0) as u64)
        .map(|(ind, _)| ind)
        .unwrap();

    let candidate1 = &population.normalised_particles()[*alpha_mean];

    let min_alpha = &population
        .normalised_particles()
        .iter()
        .enumerate()
        .min_by_key(|(_, p)| (p.parameters.alpha * 100000.0) as u64)
        .map(|(ind, _)| ind)
        .unwrap();

    let candidate2 = &population.normalised_particles()[*min_alpha];
    let min_beta = &population
        .normalised_particles()
        .iter()
        .enumerate()
        .min_by_key(|(_, p)| (p.parameters.beta * 100000.0) as u64)
        .map(|(ind, _)| ind)
        .unwrap();
    let candidate3 = &population.normalised_particles()[*min_beta];

    let particles = population.normalised_particles();

    get_samples(candidate1, particles, "1")?;
    get_samples(candidate2, particles, "2")?;
    get_samples(candidate3, particles, "3")?;

    Ok(())
}

fn get_samples(
    candidate: &Particle<TestParams>,
    particles: &Vec<Particle<TestParams>>,
    identifier: &str,
) -> ABCDResult<()> {
    let builder: OLCMKernelBuilder<TestParams> = OLCMKernelBuilder::new(particles)?;
    let mut rng = SmallRng::from_entropy();

    // Get samples
    let params = &candidate.parameters;
    let olcm = builder.build_kernel_around_parameters(params)?;

    // Generate some samples
    let samples: Vec<TestParams> = (1..=1000)
        .map(|_| olcm.perturb(&candidate.parameters, &mut rng))
        .collect::<ABCDResult<Vec<TestParams>>>()?;

    let json = json!({
        "samples": samples,
        "mean": TestParams::from_slice(&olcm.weighted_mean.iter().cloned().collect::<Vec<f64>>())
    });

    // Save them to a file
    let path = Path::new("../../out/xor_coins/samples");
    if !path.exists() {
        std::fs::create_dir_all(path)?;
    }
    let name = format!("samples_{}.json", identifier);
    std::fs::write(path.join(name), serde_json::to_string_pretty(&json)?)?;
    Ok(())
}
