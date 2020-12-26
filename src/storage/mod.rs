use std::path::PathBuf;

use crate::{Generation, Scored, Weighted};

trait Storage {
    fn check_active_gen_id(&self) -> u16;
    fn retrieve_active_gen<P>(&self) -> Generation<P>;

    fn save_particle<P>(&self,w: Weighted<P>);

    fn get_particles_if_enough<P>(&self, num_required: u16) -> Option<Scored<P>>;
    fn save_new_gen<P>(&self, g: Generation<P>);
}

struct FileSystem {
    base_path: PathBuf,
}

impl FileSystem {
    pub fn new(base_path: PathBuf) -> Self {
        FileSystem{base_path}
    }
}

impl Storage for FileSystem {
    fn check_active_gen_id(&self) -> u16 {
        let mut dir = self.base_path.clone();
        dir.push("generations");
        
        let paths = std::fs::read_dir(&dir)
            .expect(&format!("Gen directory not found: {:?}", &dir));

        let max_gen_number = paths
            .into_iter()
            .map(|p|
                p.unwrap().file_name().into_string().unwrap().parse::<u16>().unwrap()
            )
            .max()
            .unwrap();

        max_gen_number
    }

    fn retrieve_active_gen<P>(&self) -> Generation<P> {
        todo!()
    }

    fn save_particle<P>(&self, w: Weighted<P>) {
        todo!()
    }

    fn get_particles_if_enough<P>(&self, num_required: u16) -> Option<Scored<P>> {
        todo!()
    }

    fn save_new_gen<P>(&self, g: Generation<P>) {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    // Note this useful idiom: importing names from outer (for mod tests) scope.
    use super::*;

    #[test]
    fn test_zero_if_no_gen_files() {
        let mut d = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        d.push("resources/test");
        // println!("{:?}", d);

        let storage = FileSystem::new(d.clone());
        assert_eq!(0, storage.check_active_gen_id());
    }
}