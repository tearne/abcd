use std::{path::Path, process::Command};

use abcd::{error::ABCDResult, types::Vectorable, Generation};
use nalgebra::{SMatrix, Vector2};
use rand::distributions::Distribution;
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Deserialize, Debug)]
struct TestParams {
    x: f64,
    y: f64,
}

impl Vectorable<2> for TestParams {
    fn to_column_vector(&self) -> SMatrix<f64, 2, 1> {
        Vector2::new(self.x, self.y)
    }
}

pub fn main() -> ABCDResult<()> {
    let path = "resources/test/olcm/particles.json";
    let generation: Generation<TestParams> =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();

    let population = generation.pop;
    let candidate = &population.normalised_particles()[0];

    let olcm = population.olcm(candidate);

    //TODO use ThreadRng or SmallRng?
    //https://rust-random.github.io/rand/rand/rngs/index.html#:~:text=OsRng%20is%20a%20stateless%20interface,with%20periodic%20seeding%20from%20OsRng%20.
    let mut rng = rand::rngs::OsRng;
    let dist = olcm.distribution()?;
    let samples: Vec<_> = (1..=1000)
        .map(|_| {
            let v = dist.sample(&mut rng).iter().cloned().collect::<Vec<f64>>();
            P2d { x: v[0], y: v[1] }
        })
        .collect();

    #[derive(Serialize)]
    struct P2d {
        x: f64,
        y: f64,
    }

    let json = json!({
        "samples": samples,
        "mean": P2d{x: olcm.mean[0], y: olcm.mean[1]}
    });

    let path = Path::new("out");
    if !path.exists() {
        std::fs::create_dir_all(path)?;
    }
    std::fs::write(
        path.join("samples.json"),
        serde_json::to_string_pretty(&json)?,
    )?;

    // Run the plotting script
    let output = Command::new("examples/olcm_test_plot/plot/run.sh")
        .spawn()
        .expect("Failed to execute plotting script, but still generated results data in out/ dir.")
        .wait_with_output()?;

    println!("Output from Python: {:?}", output);

    Ok(())
}
