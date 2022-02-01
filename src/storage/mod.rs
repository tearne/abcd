pub mod filesystem;
pub mod s3;
pub mod config;

use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::{Population, Particle, error::ABCDResult, Generation};
use std::fmt::Debug;

pub trait Storage {
    fn check_active_gen(&self) -> ABCDResult<u16>;

    fn retrieve_previous_gen<P>(&self) -> ABCDResult<Generation<P>>
    where
        P: DeserializeOwned + Debug;

    fn save_particle<P>(&self, w: &Particle<P>) -> ABCDResult<String>
    where
        P: Serialize + Debug;

    fn num_particles_available(&self) -> ABCDResult<u32>;

    fn retrieve_all_particles<P>(&self) -> ABCDResult<Vec<Particle<P>>>
    where
        P: DeserializeOwned + Debug;

    fn save_new_gen<P>(&self, g: Population<P>, generation_number: u16) -> ABCDResult<()>
    where
        P: Serialize + Debug;
}
