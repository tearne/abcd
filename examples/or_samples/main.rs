use std::{
    ffi::OsStr,
    fs, io,
    path::{Path, PathBuf},
};

use abcd::{
    error::{ABCDErr, ABCDResult},
    kernel::{olcm::OLCMKernelBuilder, Kernel, KernelBuilder, TrivialKernel},
    Generation, Particle,
};
use nalgebra::DVector;
use rand::{rngs::SmallRng, Rng, SeedableRng};
use serde::{Deserialize, Serialize};
use serde_json::json;
use statrs::distribution::Normal;

#[derive(Serialize, Deserialize, Debug, Copy, Clone, derive_more::Add, derive_more::Sub)]
struct ProbabilityHeads {
    alpha: f64,
    beta: f64,
}

impl ProbabilityHeads {
    pub fn from_slice(slice: &[f64]) -> Self {
        Self {
            alpha: slice[0],
            beta: slice[1],
        }
    }
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

pub fn main() -> ABCDResult<()> {
    // Find the biggest available generation to use to generate samples
    fn available_generations() -> io::Result<Vec<PathBuf>> {
        let mut generations = vec![];

        for path in fs::read_dir("../../out/or_coins_olcm")? {
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
    let generation: Generation<ProbabilityHeads> =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    let population = generation.pop;

    let candidate1 = {
        let middle = &population
            .normalised_particles()
            .iter()
            .enumerate()
            .min_by_key(|(_, p)| (((p.parameters.alpha - 0.5).powf(2.0)+(p.parameters.beta - 0.5).powf(2.0)).sqrt() * 100000.0) as u64)
            .map(|(ind, _)| ind)
            .unwrap();

        &population.normalised_particles()[*middle]
    };

    let candidate2 = {
        let right = &population
            .normalised_particles()
            .iter()
            .enumerate()
            .min_by_key(|(_, p)| ((p.parameters.alpha - 0.65).abs() * 100000.0) as u64)
            .map(|(ind, _)| ind)
            .unwrap();

        &population.normalised_particles()[*right]
    };

    let candidate3 = {
        let left = &population
            .normalised_particles()
            .iter()
            .enumerate()
            .min_by_key(|(_, p)| ((p.parameters.alpha - 0.35).abs() * 100000.0) as u64)
            .map(|(ind, _)| ind)
            .unwrap();

        &population.normalised_particles()[*left]
    };

    let particles = population.normalised_particles();
    
    get_trivial_samples(candidate1)?; // get trivial kernel around middle point
    get_olcm_samples(candidate1, particles, "1")?;
    get_olcm_samples(candidate2, particles, "2")?;
    get_olcm_samples(candidate3, particles, "3")?;

    Ok(())
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

impl Kernel<ProbabilityHeads> for NormalKernel {
    fn perturb(&self, p: &ProbabilityHeads, rng: &mut impl Rng) -> ABCDResult<ProbabilityHeads> {
        let alpha: f64 = p.alpha + rng.sample(self.normal);
        let beta = p.beta + rng.sample(self.normal);

        Ok(ProbabilityHeads { alpha, beta })
    }

    fn pert_density(&self, from: &ProbabilityHeads, to: &ProbabilityHeads) -> f64 {
        let pert_density: f64 = {
            use statrs::distribution::Continuous;
            self.normal.pdf(from.alpha - to.alpha) * self.normal.pdf(from.beta - to.beta)
        };
        pert_density
    }
}


fn get_trivial_samples(candidate: &Particle<ProbabilityHeads>) -> ABCDResult<()> {
    let builder = TrivialKernel::from(NormalKernel::new(0.1));

    let mut rng = SmallRng::from_entropy();

    // Get samples
    let params = &candidate.parameters;
    let kernel = builder.build_kernel_around_parameters(params)?;

    // Generate some samples
    let samples: Vec<ProbabilityHeads> = (1..=1000)
        .map(|_| kernel.perturb(&candidate.parameters, &mut rng))
        .collect::<ABCDResult<Vec<ProbabilityHeads>>>()?;

    let json = json!({
        "samples": samples,
    });

    // Save them to a file
    let path = Path::new("../../out/samples");
    if !path.exists() {
        std::fs::create_dir_all(path)?;
    }
    let name = format!("trivial.json");
    std::fs::write(path.join(name), serde_json::to_string_pretty(&json)?)?;
    Ok(())
}

fn get_olcm_samples(
    candidate: &Particle<ProbabilityHeads>,
    particles: &Vec<Particle<ProbabilityHeads>>,
    identifier: &str,
) -> ABCDResult<()> {
    let builder: OLCMKernelBuilder<ProbabilityHeads> = OLCMKernelBuilder::new(particles)?;
    let mut rng = SmallRng::from_entropy();

    // Get samples
    let params = &candidate.parameters;
    let olcm = builder.build_kernel_around_parameters(params)?;

    // Generate some samples
    let samples: Vec<ProbabilityHeads> = (1..=1000)
        .map(|_| olcm.perturb(&candidate.parameters, &mut rng))
        .collect::<ABCDResult<Vec<ProbabilityHeads>>>()?;

    let json = json!({
        "samples": samples,
        "mean": ProbabilityHeads::from_slice(&olcm.weighted_mean.iter().cloned().collect::<Vec<f64>>())
    });

    // Save them to a file
    let path = Path::new("../../out/samples");
    if !path.exists() {
        std::fs::create_dir_all(path)?;
    }
    let name = format!("olcm_{}.json", identifier);
    std::fs::write(path.join(name), serde_json::to_string_pretty(&json)?)?;
    Ok(())
}
