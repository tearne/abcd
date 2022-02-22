use std::{
    fs::{DirEntry, File},
    io::BufReader,
    path::PathBuf,
};

use regex::Regex;
use serde::{de::DeserializeOwned, Serialize};

use crate::{
    error::{ABCDError, ABCDResult},
    Generation, Particle,
};
use uuid::Uuid;

use super::Storage;

#[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
pub struct FileSystem {
    base_path: PathBuf,
}
impl FileSystem {
    pub fn new(base_path: PathBuf) -> Self {
        Self { base_path }
    }

    fn get_particle_files_in_current_gen_folder(&self) -> ABCDResult<Vec<std::fs::DirEntry>> {
        let gen_no = self.previous_gen_number()? +1;
        println!("Active gen is {}", gen_no);
        let gen_dir = format!("gen_{:03}", gen_no);
        let dir = self.base_path.join(gen_dir);
        println!("---> {:?}", dir);

        let re = Regex::new(r#"^gen_(?P<gid>\d*)$"#)?;

        let files = std::fs::read_dir(dir)?
            //TODO use filter_map
            // .map(|r| r.map_err(crate::error::Error::from))
            .filter(|entry| {
                let entry = entry.as_ref().unwrap();
                let entry_path = entry.path();
                let filename = entry_path.file_name().unwrap();
                let file_name_as_str = filename.to_string_lossy();
                !re.is_match(&file_name_as_str)
            })
            .filter_map(Result::ok)
            .collect::<Vec<DirEntry>>();

        Ok(files)
    }
}

impl Storage for FileSystem {
    fn previous_gen_number(&self) -> ABCDResult<u16> {
        let re = Regex::new(r#"^gen_(?P<gid>\d*)$"#)?;
        let gen_non_zero_re = {
            let string = format!(r#"^{:?}/gen_(?P<gid1>\d*)/gen_(?P<gid2>\d*).json"#, &self.base_path);
            Regex::new(&string)?
        };
        let gen_zero_file = self.base_path.join("abcd.init");

        if !gen_zero_file.exists(){
            return Err(ABCDError::StorageInitError)
        }

        let gen_dirs: Vec<u16> = std::fs::read_dir(&self.base_path)?
            .filter_map(|read_dir| {
                let path = read_dir.as_ref().ok()?.path();
                println!("Path is  {:?}", path);
                if path.exists(){
                    path.file_name()
                        .map(|name| name.to_string_lossy().to_string())
                } else {
                    None
                }
            })
            .filter(|dir_name| dir_name.starts_with("gen_"))
            .filter_map(|dir_name| {
                if let Some(caps) = gen_non_zero_re.captures(&dir_name) {
                    caps["gid1"].parse::<u16>().ok()
                } else {
                    None
                }
            })
            .collect();

            /**
             *         let gen_number = key_strings
            .filter_map(|key| {
                self.gen_non_zero_re.captures(&key)
                    .map(|caps| caps["gid1"].parse::<u16>().ok())
                    .flatten()
            })
            .max()
            .unwrap_or(0);
             */

        //TODO need the equiv in the S3 Storage module
      //  if !gen_dirs.contains(&0) {
      //      Err(ABCDError::StorageInitError)
      //  } else {
          gen_dirs
                .iter()
                .max() 
                .copied()
                 .ok_or_else(|| ABCDError::Other("Failed to find max gen".into()))
       // }

    }

    fn load_previous_gen<P>(&self) -> ABCDResult<Generation<P>>
    where
        P: DeserializeOwned,
    {
        let prev_gen_no = self.previous_gen_number()?;
        let previous_gen_dir = self.base_path.join(format!("gen_{:03}", prev_gen_no));
        let file_path = previous_gen_dir.join(format!("gen_{:03}.json", prev_gen_no));
        let file = File::open(file_path)?;
        let reader = BufReader::new(file);

        //TODO why isn't our conversion in error.rs being applied?
        // let gen: Result<Generation<P>> = serde_json::from_reader(reader);

        let gen: Generation<P> = serde_json::from_reader(reader)?;

        Ok(gen)
    }

    fn save_particle<P: Serialize>(&self, w: &Particle<P>) -> ABCDResult<String> {
        let file_uuid = Uuid::new_v4();
        let file_path = self.base_path.join(file_uuid.to_string() + ".json");

        let pretty_json = serde_json::to_string_pretty(w);
        std::fs::write(&file_path, pretty_json?)?;

        Ok(file_path.to_string_lossy().into_owned())
    }

    fn num_working_particles(&self) -> ABCDResult<u32> {
        let files_in_folder = self.get_particle_files_in_current_gen_folder();

        match files_in_folder {
            Err(_) if self.previous_gen_number().ok() == Some(0) => Ok(0),
            Ok(files) => Ok(files.len() as u32), //TODO read dir numbers & take max //TODO safer way to do cast - Ok(u16::try_from(file.len()))
            Err(e) => Err(e),
        }
    }

    fn load_working_particles<P: DeserializeOwned>(&self) -> ABCDResult<Vec<Particle<P>>> {
        let particle_files = self.get_particle_files_in_current_gen_folder()?;

        let mut weighted_particles = Vec::new();
        for entry in particle_files {
            let file = File::open(entry.path())?;
            let reader = BufReader::new(file);
            let wp: Particle<P> = serde_json::from_reader(reader)?;
            weighted_particles.push(wp);
        }

        Ok(weighted_particles)
    }

    fn save_new_gen<P: Serialize>(&self, gen: &Generation<P>) -> ABCDResult<()> {
        let gen_dir = self.base_path.join(format!("gen_{:03}", gen.number));
        let file_path = gen_dir.join(format!("gen_{:03}.json", gen.number));

        match file_path.exists() {
            false => {
                let serialised_gen = serde_json::to_string_pretty(&gen);
                std::fs::write(&file_path, serialised_gen?)?;
                Ok(())
            }
            true => Err(ABCDError::GenAlreadySaved(format!(
                "Gen file already existed at {:?}",
                file_path
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use serde::{Deserialize, Serialize};
    use serde_json::Value;
    use std::path::{Path, PathBuf};

    use crate::{error::ABCDError, storage::test_helper::make_dummy_generation, types::Population};

    use super::*;

    struct TmpDir {
        path: PathBuf,
        delete_on_drop: bool,
    }
    impl TmpDir {
        pub fn new(name: &str, delete_on_drop: bool) -> Self {
            let path = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("target")
                .join("test_tmp")
                .join(name);
            if path.exists() {
                std::fs::remove_dir_all(&path).unwrap();
            }
            std::fs::create_dir_all(&path).expect("failed to create");
            TmpDir {
                path,
                delete_on_drop,
            }
        }
    }
    impl Drop for TmpDir {
        fn drop(&mut self) {
            if self.delete_on_drop {
                std::fs::remove_dir_all(&self.path).unwrap();
            }
        }
    }

    #[derive(Serialize, Deserialize, Debug, PartialEq)]
    struct DummyParams {
        a: u16,
        b: f32,
    }
    impl DummyParams {
        pub fn new(a: u16, b: f32) -> Self {
            DummyParams { a, b }
        }
    }

    fn manifest_dir() -> &'static Path {
        // From: https://doc.rust-lang.org/cargo/reference/environment-variables.html
        Path::new(env!("CARGO_MANIFEST_DIR"))
    }

    fn make_dummy_population() -> Population<DummyParams> {
        let particle_1 = Particle {
            parameters: DummyParams::new(10, 20.),
            scores: vec![1000.0, 2000.0],
            weight: 0.234,
        };

        let particle_2 = Particle {
            parameters: DummyParams::new(30, 40.),
            scores: vec![3000.0, 4000.0],
            weight: 0.567,
        };

        Population {
            tolerance: 0.1234,
            acceptance: 0.7,
            normalised_particles: vec![particle_1, particle_2],
        }
    }

    #[test]
    fn test_check_active_gen() {
        let base_path = manifest_dir().join("resources/test/storage/example");
        let storage = FileSystem::new(base_path);
        assert_eq!(3, storage.previous_gen_number().unwrap());
    }

    #[test]
    fn test_retrieve_previous_gen() {
        let expected = Generation {
            number: 2,
            pop: make_dummy_population(),
        };

        let base_path = manifest_dir().join("resources/test/storage/example/");
        let instance = FileSystem::new(base_path);

        let result = instance.load_previous_gen::<DummyParams>().unwrap();

        assert_eq!(expected, result);
    }

    #[test]
    fn test_save_particle() {
        let tmp_dir = TmpDir::new("save_particle", true);
        let storage = FileSystem::new(tmp_dir.path.clone());

        let p1 = DummyParams::new(1, 2.);
        let p2 = DummyParams::new(3, 4.);

        let w1 = Particle {
            parameters: p1,
            scores: vec![100.0, 200.0],
            weight: 1.234,
        };

        let w2 = Particle {
            parameters: p2,
            scores: vec![300.0, 400.0],
            weight: 1.567,
        };

        let saved_1 = storage.save_particle(&w1).unwrap();
        let _saved_2 = storage.save_particle(&w2).unwrap();

        let file = std::fs::File::open(tmp_dir.path.clone().join(saved_1)).unwrap();
        let loaded: Particle<DummyParams> =
            serde_json::from_reader(std::io::BufReader::new(file)).unwrap();

        assert_eq!(w1, loaded);
    }

    //expection if
    // - try to save gen but there's a higher number gen in there already
    // - gen already exists

    #[test]
    fn test_exception_when_empty_folder() {
        //Actually turn this into test for active Gen = 0?
        let full_path = manifest_dir().join("resources/test/storage/empty/");
        let storage = FileSystem::new(full_path);

        let expected_message = "No Gen Zero Directory Exists";

        //TODO helper function to remove duplication?
        match storage.previous_gen_number() {
            Ok(_) => panic!("Expected error"),
            Err(ABCDError::StorageInitError) => (),
            Err(e) => panic!("Wrong error, got: {}", e),
        };

        match storage.num_working_particles() {
            Ok(_) => panic!("Expected error"),
            Err(ABCDError::StorageInitError) => (),
            Err(e) => panic!("Wrong error, got: {}", e),
        };

        match storage.load_working_particles::<DummyParams>() {
            Ok(_) => panic!("Expected error"),
            Err(ABCDError::StorageInitError) => (),
            Err(e) => panic!("Wrong error, got: {}", e),
        };
    }

    #[test]
    fn test_number_particle_files() {
        let full_path = manifest_dir().join("resources/test/storage/example/");
        let storage = FileSystem::new(full_path);
        assert_eq!(2, storage.num_working_particles().unwrap())
    }

    #[test]
    fn test_retrieve_particle_files() {
        let full_path = manifest_dir().join("resources/test/storage/example/");
        let instance = FileSystem::new(full_path);

        let mut expected = {
            let w1 = Particle {
                parameters: DummyParams::new(1, 2.),
                scores: vec![100.0, 200.0],
                weight: 1.234,
            };

            let w2 = Particle {
                parameters: DummyParams::new(3, 4.),
                scores: vec![300.0, 400.0],
                weight: 1.567,
            };

            vec![w1, w2]
        };

        let mut result: Vec<Particle<DummyParams>> = instance.load_working_particles().unwrap();

        //Sort by weight for easy comparison
        expected.sort_by(|a, b| a.weight.partial_cmp(&b.weight).unwrap());
        result.sort_by(|a, b| a.weight.partial_cmp(&b.weight).unwrap());

        assert_eq!(expected, result);
    }

    #[test]
    fn save_new_generation() {
        let tmp_dir = TmpDir::new("save_generation", true);
        let instance = FileSystem::new(tmp_dir.path.clone());

        let gen_number = 3;
        let gen_acceptance = 0.3;

        let gen = make_dummy_generation(gen_number, gen_acceptance);
        std::fs::create_dir(instance.base_path.join("gen_003"))
            .expect("Expected successful dir creation");

        instance
            .save_new_gen(&gen)
            .expect("Expected successful save");

        //   let expected = serde_json::to_string_pretty(&gen).unwrap(); //Doesn't seem to compare will when prettyified


        let expected = serde_json::json!({
            "number": 3,
            "pop": {
            "tolerance": 0.5678,
            "acceptance": 0.3,
            "normalised_particles": [
                {
                    "parameters" : {
                        "a": 11, "b": 22.0
                    },
                    "scores": [1111.0, 2222.0],
                    "weight": 0.89
                },{
                    "parameters" : {
                        "a": 33, "b": 44.0
                    },
                    "scores": [3333.0, 4444.0],
                    "weight": 0.10
                }
            ]
            }
        });

        let actual = {
            let file = File::open(&tmp_dir.path.join("gen_003").join("gen_003.json")).unwrap();
            println!("Trying to load gen from {:?}", file);
            let reader = BufReader::new(file);
            serde_json::from_reader::<_, Value>(reader).unwrap()
        };

        assert_eq!(expected, actual);
    }

    #[test]
    fn dont_save_over_existing_gen_file() {
        let tmp_dir = TmpDir::new("save_over_generation", true);
        let instance = FileSystem::new(tmp_dir.path.clone());

        let gen_number = 4;
        let dummy_gen_1 = make_dummy_generation(gen_number, 0.3);
        let dummy_gen_2 = make_dummy_generation(gen_number, 0.4);

        //1. Save an dummy gen_003 file, representing file already save by another node
        std::fs::create_dir(instance.base_path.join("gen_004"))
            .expect("Expected successful dir creation");

        let outcome1 = instance.save_new_gen(&dummy_gen_1);
        match outcome1 {
            Ok(_) => (),
            Err(e) => panic!("Wrong error, got: {}", e), //Note seems to be deleted at this point?
        }

        //2. Try to save another gen over it, pretending we didn't notice the other node save gen before us
        let outcome2 = instance.save_new_gen(&dummy_gen_2);
        match outcome2 {
            Ok(_) => panic!("Expected error"),
            Err(ABCDError::GenAlreadySaved(_)) => (),
            Err(e) => panic!("Wrong error, got: {}", e), //Note seems to be deleted at this point?
        }

        //3. Test that the original file save by other node is intact.
        let loaded = {
            let string =
                std::fs::read_to_string(tmp_dir.path.join("gen_004").join("gen_004.json")).unwrap();
            serde_json::from_str(&string).unwrap()
        };
        assert_eq!(dummy_gen_1, loaded);
    }
}
