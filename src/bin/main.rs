use abcd::{Model, Random};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
struct MyParameters {
    p_heads: f64,
}

#[derive(Debug)]
struct MyModel {
    name: String,
}

impl Model for MyModel {
    type Parameters = MyParameters;

    fn prior_sample(&self, _random: &Random) -> Self::Parameters {
        todo!()
    }

    fn prior_density(&self, _p: &Self::Parameters) -> f64 {
        todo!()
    }

    fn perturb(&self, _p: &Self::Parameters) -> Self::Parameters {
        todo!()
    }

    fn pert_density(&self, _a: &Self::Parameters, _b: &Self::Parameters) -> f64 {
        todo!()
    }

    fn score(&self, _p: &Self::Parameters) -> f64 {
        // let parameter_args: Vec<String> = _p.to_args();
        // // vec!["beta=0.23213123", "badger=123.23", ...]
        // let out = Command::new("tbmi-run.sh")
        //     .args(parameter_args)
        //     .run()
        //     .output()?...

        todo!()
    }
}

fn main() {
    let m = MyModel {
        name: "Awesome Model".to_string(),
    };

    // abcd::run(m, config);
    println!("{:?}", m);
}
