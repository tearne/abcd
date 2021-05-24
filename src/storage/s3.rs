use serde::{Serialize, de::DeserializeOwned};

use crate::{Generation, Particle};
use crate::error::{Error, Result};
use super::Storage;

struct S3 {

}
impl Storage for S3 {
    fn check_active_gen(&self) -> Result<u16> {
        unimplemented!();
    }

    fn retrieve_previous_gen<'de, P>(&self) -> Result<Generation<P>> where P: DeserializeOwned{
        unimplemented!();
    }
    fn save_particle<P: Serialize>(&self, w: &Particle<P>) -> Result<String>{
        unimplemented!();
    }
    fn num_particles_available(&self) -> Result<u16>{
        unimplemented!();
    }
    fn retrieve_all_particles<P>(&self) -> Result<Vec<Particle<P>>> where P: DeserializeOwned{
        unimplemented!();
    }
    fn save_new_gen<P: Serialize>(&self, g: Generation<P>) -> Result<()>{
        unimplemented!();
    }
}