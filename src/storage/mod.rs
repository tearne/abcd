pub mod filesystem;
pub mod s3;
pub mod config;

use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::{Population, Particle};
use std::fmt::Debug;
use anyhow::Result;

pub trait Storage {
    fn check_active_gen(&self) -> Result<u16>;

    fn retrieve_previous_gen<P>(&self) -> Result<Population<P>>
    where
        P: DeserializeOwned + Debug;

    fn save_particle<P>(&self, w: &Particle<P>) -> Result<String>
    where
        P: Serialize + Debug;

    fn num_particles_available(&self) -> Result<u16>;

    fn retrieve_all_particles<P>(&self) -> Result<Vec<Particle<P>>>
    where
        P: DeserializeOwned + Debug;

    fn save_new_gen<P>(&self, g: Population<P>) -> Result<()>
    where
        P: Serialize + Debug;
}
