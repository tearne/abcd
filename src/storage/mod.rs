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

impl Storage for FileSystem {
    fn check_active_gen_id(&self) -> u16 {
        let mut dir = self.base_path.clone();
        dir.push("gen");
        
        let paths = std::fs::read_dir(&dir)
            .expect(&format!("Generations sub-dir not found: {:?}", &dir));

        let max_gen_number = paths
            .into_iter()
            .map(|p|
                p.ok()
                    .and_then(|v|v.file_name().into_string().ok())
                    .filter(|v|!v.starts_with("."))
                    .and_then(|v|v.parse::<u16>().ok())
            )
            .flatten()
            .max()
            .unwrap_or_default();

        max_gen_number
    }

    fn retrieve_active_gen<P>(&self) -> Generation<P> {
        todo!()
    }

    fn save_particle<P>(&self, _w: Weighted<P>) {
        todo!()
    }

    fn get_particles_if_enough<P>(&self, _num_required: u16) -> Option<Scored<P>> {
        todo!()
    }

    fn save_new_gen<P>(&self, _g: Generation<P>) {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_no_gen_files() {
        // From: https://doc.rust-lang.org/cargo/reference/environment-variables.html
        let mut d = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        d.push("resources/test/fs/empty");

        let storage = FileSystem{base_path: d.clone()};
        assert_eq!(0, storage.check_active_gen_id());
    }
}