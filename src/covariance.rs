

#[cfg(test)]
mod tests {
    use nalgebra::{Matrix2, SMatrix, Vector2};
    use serde::Deserialize;

    use crate::{types::Vectorable, Generation};

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
        let path = "resources/test/olcm/particles.json";
        let generation: Generation<TestParams> = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();

        let population = generation.pop;
        let candidate = &population.normalised_particles()[0];

        let olcm = population.olcm(candidate);
        assert_eq!(Matrix2::new(4.8, -13.6, -13.6, 44.1), olcm.local_covariance);
        assert_eq!(Vector2::new(10.0, 100.1), olcm.mean);
    }
}