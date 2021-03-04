use std::{fs::DirEntry, path::{Path, PathBuf}};

use regex::Regex;
use serde::Serialize;

use crate::{Generation, Weighted, error::Error};
use crate::error::Result;
use uuid::Uuid;
use std::convert::TryInto;

trait Storage {
    fn check_active_gen_id(&self) -> Result<u16>;
    fn retrieve_active_gen<P>(&self) -> Result<Generation<P>>;

    fn save_particle<P: Serialize>(&self, w: &Weighted<P>) -> Result<String>;

    fn num_particles_available(&self) -> Result<u16>;
    fn retrieve_all_particles<P>(&self) -> Vec<Weighted<P>>;

    fn save_new_gen<P>(&self, g: Generation<P>) -> Result<u16>;
}

struct FileSystem<'a> {
    base_path: &'a Path,
}

impl Storage for FileSystem<'_> {
    fn check_active_gen_id(&self) -> Result<u16> {
        //let dir = self.base_path.join("gen");
        
        let re = Regex::new(r#"^gen_(?P<gid>\d*)$"#).unwrap();

        let entries: Vec<_> = std::fs::read_dir(self.base_path)?
            // .into_iter()
            .filter(|f| {
                let path = f.as_ref().unwrap().path();
                //TODO this stinks
                path.is_dir() && path.file_name().unwrap().to_string_lossy().starts_with("gen_")
            })
            .map(|f| f.as_ref().unwrap().path().file_name().unwrap().to_string_lossy().to_string())
            .map(|dir_name| {
                let caps = re.captures(&dir_name).unwrap();
                let genid = &caps["gid"];

            })
            .collect();

        println!("=====> {:?}", entries);

        //unimplemented!();

        // let entries: std::result::Result<Vec<DirEntry>, std::io::Error> = 
        //         std::fs::read_dir(&dir)?
        //         .into_iter()
        //         .collect();

        // let entry_names: std::result::Result<Vec<String>, std::ffi::OsString> = 
        //     entries?.into_iter()
        //         .map(|v| v.file_name().into_string())
        //         .collect();

        // let generation_numbers: std::result::Result<Vec<u16>, std::num::ParseIntError>  = 
        //     entry_names?
        //         .into_iter()
        //         .filter(|v| !v.starts_with("."))
        //         // .filter(|v|???) Filter out dirs?  Test me?
        //         .map(|v| v.parse::<u16>())
        //         .collect();

        // let files = std::fs::read_dir(self.base_path).unwrap();

        // let gen_dirs = 
        // files.
        // filter(|f| f.path().unwrap().starts_with("gen"))
        // .collect();
        // Result::Ok(gen_dirs.count).unwrap_or(0))

       // Result::Ok(generation_numbers?.into_iter().max().unwrap_or(0))
       Result::Ok(entries.len().try_into().unwrap())
    }

    fn retrieve_active_gen<P>(&self) -> Result<Generation<P>> {
        todo!()
    }

    fn save_particle<P: Serialize>(&self, w: &Weighted<P>) -> Result<String> {
        let file_uuid = Uuid::new_v4();
        let file_path = self.base_path.join(file_uuid.to_string()+".json");

        let pretty_json = serde_json::to_string_pretty(w);
        std::fs::write(&file_path, pretty_json?)?;
        
        Ok(file_path.to_string_lossy().into_owned())
    }

    fn num_particles_available(&self) -> Result<u16> {
        todo!()
        // // Get current non finished gen?
        // let gen_no = self.check_active_gen_id().to_string();
        // let dir = self.base_path.join(gen_no);
        // let files = std::fs::read_dir(dir).unwrap();
        // let particle_file_names = 
        //          files
        //              .filter(|v| v.path().extension() == Some(OsStr::from_bytes(b"json")))
        //        //      .for_each(|f| println!("{:?}",f));
        //              .collect();
        //  Result::Ok(particle_file_names.count).unwrap_or(0))
    }

    fn retrieve_all_particles<P>(&self) -> Vec<Weighted<P>> {
        todo!()
    }

    fn save_new_gen<P>(&self, _g: Generation<P>) -> Result<u16> {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use crate::Scored;
    use serde::{Serialize, Deserialize};

    use super::*;

    struct TmpDir(PathBuf);
    impl TmpDir {
        pub fn new(name: &str) -> Self {
            let tmp_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("target").join("test_tmp").join(name);
            if tmp_path.exists() {
                std::fs::remove_dir_all(&tmp_path).unwrap();
            }
            std::fs::create_dir_all(&tmp_path).expect("failed to create");
            TmpDir(tmp_path)
        }
    }
    impl Drop for TmpDir {
        fn drop(&mut self){
            std::fs::remove_dir_all(self.0.as_path()).unwrap();
        }
    }

    #[derive(Serialize, Deserialize, Debug, PartialEq)]
    struct DummyParams{
        a: u16,
        b: f32
    }
    impl DummyParams{
        pub fn new(a: u16, b: f32) -> Self { DummyParams{a,b} }
    }

    fn manifest_dir() -> &'static Path {
        // From: https://doc.rust-lang.org/cargo/reference/environment-variables.html
        Path::new(env!("CARGO_MANIFEST_DIR"))
    }

    fn storage(p: &Path) -> FileSystem {
        FileSystem{base_path: p}
    }

    #[test]
    fn test_no_gen_files() {
        let full_path = manifest_dir().join("resources/test/fs/empty");
        let storage = storage(&full_path);
        assert_eq!(0, storage.check_active_gen_id().unwrap());
    }

    #[test]
    fn test_number_gen_files() {
        let full_path = manifest_dir().join("resources/test/fs/example");
        let storage = storage(&full_path);
        assert_eq!(2, storage.check_active_gen_id().unwrap());
    }

    #[test]
    fn test_save_particle() {
        let tmp_dir = TmpDir::new("save_particle");

        let p1 = DummyParams::new(1,2.);
        let p2 = DummyParams::new(2,3.);
        let p3 = DummyParams::new(3,4.);
        let p4 = DummyParams::new(4,5.);

        let w1 = Weighted {
            scored_vec: vec![Scored::new(p1,1.0), Scored::new(p2,2.0)],
            weight: 3.0,
        };

        let w2 = Weighted {
            scored_vec: vec![Scored::new(p3,3.0), Scored::new(p4,4.0)],
            weight: 2.0,
        };

        let storage = storage(&tmp_dir.0);

        let saved_1 = storage.save_particle(&w1).unwrap();
        println!("File was saved to {}", saved_1);
        let saved_2 = storage.save_particle(&w2).unwrap();

        let file = std::fs::File::open(tmp_dir.0.clone().join(saved_1)).unwrap();
        println!("About to try and load from {:?}", file);
        let loaded: Weighted<DummyParams> = serde_json::from_reader(std::io::BufReader::new(file)).unwrap();
        
        assert_eq!(w1, loaded);
    }

    #[test]
    fn test_no_particle_files() {
        let full_path = manifest_dir().join("resources/test/fs/empty");
        let storage = storage(&full_path);
        assert_eq!(0,storage.num_particles_available().unwrap())
    }


    #[test]
    fn test_number_particle_files() {
        let tmp_dir = TmpDir::new("save_particle");

        let p1 = DummyParams::new(1,2.);
        let p2 = DummyParams::new(2,3.);
        let p3 = DummyParams::new(3,4.);
        let p4 = DummyParams::new(4,5.);

        let w1 = Weighted {
            scored_vec: vec![Scored::new(p1,1.0), Scored::new(p2,2.0)],
            weight: 3.0,
        };

        let w2 = Weighted {
            scored_vec: vec![Scored::new(p3,3.0), Scored::new(p4,4.0)],
            weight: 2.0,
        };

        let storage = storage(&tmp_dir.0);

        let saved_1 = storage.save_particle(&w1).unwrap();
        println!("File 1 was saved to {}", saved_1);
        let saved_2 = storage.save_particle(&w2).unwrap();
        println!("File 2 was saved to {}", saved_2);
        assert_eq!(2,storage.num_particles_available().unwrap())
    }


}