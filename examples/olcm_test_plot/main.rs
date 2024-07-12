use std::{path::Path, process::Command};

use abcd::{error::{ABCDResult, VectorConversionError}, kernel::olcm::OLCMKernelBuilder, types::Vector, Generation};
use nalgebra::{SMatrix, Vector2};
use rand::{distributions::Distribution, rngs::SmallRng, SeedableRng};
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Serialize, Deserialize, Debug, Copy, Clone, derive_more::Add, derive_more::Sub)]
struct TestParams {
    x: f64,
    y: f64,
}
impl TestParams {
    pub fn from_slice(slice: &[f64]) -> Self {
        Self {
            x: slice[0],
            y: slice[1]
        }
    }
}

impl Vector<2> for TestParams {
    fn to_column_vector(&self) -> SMatrix<f64, 2, 1> {
        Vector2::new(self.x, self.y)
    }
    
    fn from_column_vector(v: nalgebra::DVector<f64>) -> Result<Self, abcd::error::VectorConversionError> {
        let values = v.iter().cloned().collect::<Vec<f64>>();
        if values.len() != 2 {
            return Err(VectorConversionError(format!(
                "Wrong number of arguments.  Expected 2, got {}",
                values.len()
            )));
        } else {
            Ok(TestParams {
                x: values[0],
                y: values[1],
            })
        }
    }
}

pub fn main() -> ABCDResult<()> {
    let path = "resources/test/olcm/particles.json";
    let generation: Generation<TestParams> =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();

    let population = generation.pop;
    let candidate = &population.normalised_particles()[0];

    let builder = OLCMKernelBuilder::new(population.normalised_particles())?;
    let olcm = builder.build_kernel_around(candidate)?;

    let mut rng = SmallRng::from_entropy();

    // Generate some samples
    let samples: Vec<TestParams> = (1..=1000)
        .map(|_| olcm.perturb(&candidate.parameters, &mut rng))
        .collect::<ABCDResult<Vec<TestParams>>>()?;
    let json = json!({
        "samples": samples,
        "mean": TestParams::from_slice(&olcm.weighted_mean.iter().cloned().collect::<Vec<f64>>())
    });

    // Save them to a file
    let path = Path::new("out");
    if !path.exists() { std::fs::create_dir_all(path)?; }
    std::fs::write(
        path.join("samples.json"),
        serde_json::to_string_pretty(&json)?,
    )?;

    // Run the plotting script
    let output = Command::new("examples/olcm_test_plot/plot/run.sh")
        .spawn()
        .expect("Failed to execute plotting script, but still generated results data in out/ dir.")
        .wait_with_output()?;

    println!("Output from Python: {:#?}", output);

    Ok(())
}
