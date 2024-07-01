

#[cfg(test)]
mod tests {
    use std::{path::Path, process::Command};

    use nalgebra::{Matrix2, SMatrix, Vector2};
    use path_absolutize::Absolutize;
    use rand::distributions::Distribution;
    use serde::{Deserialize, Serialize};
    use serde_json::json;
    use statrs::distribution::MultivariateNormal;

    use crate::{error::ABCDResult, types::Vectorable, Generation};

    #[derive(Deserialize, Debug)]
    struct TestParams{
        x: f64,
        y: f64,
    }
    
    impl Vectorable<2> for TestParams {
        fn to_column_vector(&self) -> SMatrix<f64, 2, 1> {
           Vector2::new(self.x, self.y)
       }
    }

    #[test]
    fn test_covariance_matrix() {
        let path = "resources/test/covariance/particles.json";
        let generation: Generation<TestParams> = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();

        let population = generation.pop;
        let candidate = &population.normalised_particles()[0];

        let olcm = population.olcm(candidate);
        assert_eq!(Matrix2::new(4.8, -13.6, -13.6, 44.1), olcm.local_covariance);
        assert_eq!(Vector2::new(10.0, 100.1), olcm.mean);

        // //TODO use ThreadRng or SmallRng?
        // //https://rust-random.github.io/rand/rand/rngs/index.html#:~:text=OsRng%20is%20a%20stateless%20interface,with%20periodic%20seeding%20from%20OsRng%20.
        // let mut rng = rand::rngs::OsRng;
        // let dist = olcm.distribution()?;
        // let samples: Vec<_> = (1..=1000).map(|_| {
        //     let v = dist.sample(&mut rng).iter().cloned().collect::<Vec<f64>>();
        //     P2d{x: v[0], y: v[1]}
        // }).collect();
        
        // #[derive(Serialize)]
        // struct P2d{
        //     x: f64,
        //     y: f64,
        // }

        // let json = json!({
        //     "samples": samples,
        //     "mean": P2d{x: olcm.mean[0], y: olcm.mean[1]}
        // });
        
        // let path = Path::new("out");
        // if !path.exists() {
        //     std::fs::create_dir_all(path)?;
        // }
        // std::fs::write(path.join("samples.json"), serde_json::to_string_pretty(&json)?)?;

        // // Run the plotting script
        // Command::new("resources/test/covariance/plot/run.sh").spawn()?;

        // Ok(())
    }
}