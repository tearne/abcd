

#[cfg(test)]
mod tests {
    use nalgebra::{Matrix2, SMatrix, Vector2};
    use rand::distributions::Distribution;
    use serde::Deserialize;
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
    fn test_covariance_matrix() -> ABCDResult<()>{
        let path = "resources/test/covariance/particles.json";
        let generation: Generation<TestParams> = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();

        let population = generation.pop;
        let candidate = &population.normalised_particles()[0];

        let olcm = population.olcm(candidate);
        assert_eq!(Matrix2::new(4.8, -13.6, -13.6, 44.1), olcm.local_covariance);
        assert_eq!(Vector2::new(10.0, 100.1), olcm.mean);

        // Plot stuff
        let t: Vec<f64> = candidate.parameters.to_column_vector().iter().cloned().collect();

        let mut rng = rand::rngs::OsRng;
        let dist = olcm.distribution()?;
        let samples: Vec<_> = (1..=10).map(|_| dist.sample(&mut rng)).collect();
        samples.iter().for_each(|s|println!("{}", s));
        Ok(())
    }
}