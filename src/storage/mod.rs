mod filesystem;
mod s3;

use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::{Generation, Particle};
use crate::error::Result;

trait Storage {
    fn check_active_gen(&self) -> Result<u16>;
    fn retrieve_previous_gen<'de, P>(&self) -> Result<Generation<P>> where P: DeserializeOwned;
    fn save_particle<P: Serialize>(&self, w: &Particle<P>) -> Result<String>;
    fn num_particles_available(&self) -> Result<u16>;
    fn retrieve_all_particles<P>(&self) -> Result<Vec<Particle<P>>> where P: DeserializeOwned;
    fn save_new_gen<P: Serialize>(&self, g: Generation<P>) -> Result<()>;
}
