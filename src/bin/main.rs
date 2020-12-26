use abcd::Model;
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize)]
struct MyParameters {
    p_heads: f64,
}

#[derive(Debug)]
struct MyModel{
    name: String,
}

impl Model for MyModel{
    type Parameters = MyParameters;

    fn prior_sample<R: abcd::Random>(&self, random: &R) -> Self::Parameters {
        todo!()
    }

    fn prior_density(&self) -> f64 {
        todo!()
    }

    fn perturb(&self, p: Self::Parameters) -> Self::Parameters {
        todo!()
    }

    fn pert_density(&self, a: Self::Parameters, b: Self::Parameters) -> f64 {
        todo!()
    }

    fn score(&self, p: Self::Parameters) -> f64 {
        todo!()
    }
}

fn main() {
    let m = MyModel{
        name: "Awesome Model".to_string(),
    };

    println!("{:?}", m);
}