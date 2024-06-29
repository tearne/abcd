

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
        let expected_cov = Matrix2::new(4.8, -13.6, -13.6, 44.1);
        println!("Expected covariance {}", expected_cov);

        let expected_mean = Vector2::new(10.0, 100.1);
        println!("Expected mean {}", expected_mean);

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
            (mean - candidate) * (mean - candidate).transpose()
        };

        println!("Local cov {}", local_cov);

        assert_eq!(expected_mean, mean);
        assert_eq!(expected_cov, local_cov);
    }

    #[test]
    fn test_weighted_mean(){
        let expected = Vector2::new(10.0, 100.1);

        let path = "resources/test/covariance/particles.json";
        let generation: Generation<Params<2>> = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();

        let calculated = generation.pop.normalised_particles().iter().fold(Vector2::<f64>::zeros(), |acc, particle|{
            let parameters_vec = particle.parameters.to_column_vector();
            let weight = particle.weight;
            acc + weight * parameters_vec
        });

        println!("mean = {}", calculated);

        assert_eq!(expected, calculated)


        // let mut param_col_vectors: Vec<Vector2<f64>> = Vec::new();
        // for p in generation.pop.normalised_particles(){
        //     param_col_vectors.push(p.parameters.to_column_vector());
        // }

        // println!("There are {} columns", param_col_vectors.len());
        // param_col_vectors.iter().for_each(|e|println!("{}", e));

        // let mean_1 = param_col_vectors.iter().fold(Vector2::<f64>::zeros(), |acc, next|{
        //     acc + next
        // }).component_div(&Vector2::<f64>::from_element(param_col_vectors.len() as f64));

        // let mean_2 = Matrix2xX::from_columns(&param_col_vectors).column_mean();

        // println!("{:?}", mean_1);
        // println!("{:?}", mean_2);

        
        // let m: Vector2<f64> = param_col_vectors[0];
        // let n: Vector2<f64> = param_col_vectors[1];
        // println!("m: {:?}", m);
        // println!("n: {:?}", n);
        // let r =  Vector2::from_iterator(m.into_iter().zip(n.into_iter()).map(|(a,b)| a + 2.0*b));
        // println!("r: {:?}", r);

        // let m: Vector2<f64> = param_col_vectors.iter().sum::<Vector2<f64>>().into_iter().map(|mut e| e / (n as f64)).collect();
        // let m: Vector2<f64> = param_col_vectors.iter().sum::<Vector2<f64>>(). / param_col_vectors.len();
        // println!("mean: {:#?}", m);


        // println!("Generation = {:#?}", generation);
    }


}