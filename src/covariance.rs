

#[cfg(test)]
mod tests {
    use nalgebra::{DMatrix, Matrix, Matrix2, Matrix2xX, SMatrix, Vector2};
    use serde::Deserialize;

    use crate::Generation;

    #[derive(Deserialize, Debug)]
    struct Params<const D: usize>{
        x: f64,
        y: f64,
    }
    trait Vector<const D: usize>{
        fn to_column_vector(&self) -> SMatrix<f64, D, 1>;
    }

    impl Vector<2> for Params<2> {
         fn to_column_vector(&self) -> SMatrix<f64, 2, 1> {
            Vector2::new(self.x, self.y)
        }
    }


    #[test]
    fn test_covariance_matrix(){
        let path = "resources/test/covariance/particles.json";
        let generation: Generation<Params<2>> = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();

        let mean = generation.pop.normalised_particles().iter().fold(Vector2::<f64>::zeros(), |acc, particle|{
            let parameters_vec = particle.parameters.to_column_vector();
            let weight = particle.weight;
            acc + weight * parameters_vec
        });

        let candidate = generation.pop.normalised_particles()[0].parameters.to_column_vector();

        let local_cov = {
            let cov = generation.pop.normalised_particles().iter().fold(Matrix2::zeros(), |acc, par|{
                let params = par.parameters.to_column_vector();
                let weight = par.weight;

                acc + weight * (params - mean) * (params - mean).transpose()
            });
            cov + (mean - candidate) * (mean - candidate).transpose()
        };

        assert_eq!(Vector2::new(10.0, 100.1), mean);
        assert_eq!(Matrix2::new(4.8, -13.6, -13.6, 44.1), local_cov);
    }
}