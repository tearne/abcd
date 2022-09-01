pub mod config;

pub mod s3;

use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::{error::ABCDResult, Generation, Particle};
use std::fmt::Debug;

pub trait Storage {
    fn previous_gen_number(&self) -> ABCDResult<u16>;

    fn load_previous_gen<P>(&self) -> ABCDResult<Generation<P>>
    where
        P: DeserializeOwned + Debug;

    fn save_particle<P>(&self, particle: &Particle<P>, gen_num: u16) -> ABCDResult<String>
    where
        P: Serialize + Debug;

    fn num_accepted_particles(&self) -> ABCDResult<u32>;

    fn load_accepted_particles<P>(&self) -> ABCDResult<Vec<Particle<P>>>
    where
        P: DeserializeOwned + Debug;

    fn num_rejected_particles(&self) -> ABCDResult<u64>;

    fn save_new_gen<P>(&self, generation: &Generation<P>) -> ABCDResult<()>
    where
        P: Serialize + Debug;
}

#[cfg(test)]
mod test_helper {
    use serde::{Deserialize, Serialize};

    use crate::{types::Population, Generation, Particle};

    #[derive(Serialize, Deserialize, Debug, PartialEq)]
    pub struct DummyParams {
        a: u16,
        b: f32,
    }
    impl DummyParams {
        pub fn new(a: u16, b: f32) -> Self {
            DummyParams { a, b }
        }
    }

    pub fn gen_002() -> Generation<DummyParams> {
        Generation {
            pop: Population::new(
                vec![
                    Particle {
                        parameters: DummyParams::new(10, 20.0),
                        scores: vec![1000.0, 2000.0],
                        weight: 0.2,
                    },
                    Particle {
                        parameters: DummyParams::new(30, 40.0),
                        scores: vec![3000.0, 4000.0],
                        weight: 0.8,
                    },
                ],
                0.7,
            ),
            next_gen_tolerance: 0.1234,
            number: 2,
        }
    }

    pub fn make_dummy_generation(gen_number: u16) -> Generation<DummyParams> {
        let particle_1 = Particle {
            parameters: DummyParams::new(11, 22.),
            scores: vec![1111.0, 2222.0],
            weight: 0.9,
        };

        let particle_2 = Particle {
            parameters: DummyParams::new(33, 44.),
            scores: vec![3333.0, 4444.0],
            weight: 0.1,
        };

        let pop = Population::new(vec![particle_1, particle_2], 0.75);

        Generation {
            pop,
            number: gen_number,
            next_gen_tolerance: 0.1234
        }
    }
}
