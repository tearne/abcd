use std::{
    fs::{DirEntry, File},
    io::BufReader,
    path::Path,
};

use regex::Regex;
use serde::{de::DeserializeOwned, Serialize};

use crate::error::Result;
use crate::{error::Error, Generation, Particle};
use uuid::Uuid;

use super::Storage;

struct FileSystem<'a> {
    base_path: &'a Path,
}
impl<'a> FileSystem<'a> {
    fn get_particle_files_in_current_gen_folder(&self) -> Result<Vec<std::fs::DirEntry>> {
        //TODO test case for when returns 1?
        let gen_no = self.check_active_gen().unwrap_or(1);
        println!("Active gen is {}", gen_no);
        let gen_dir = format!("gen_{:03}", gen_no);
        let dir = self.base_path.join(gen_dir);
        println!("---> {:?}", dir);

        let re = Regex::new(r#"^gen_(?P<gid>\d*)$"#).unwrap(); //TODO use ?

        let r = std::fs::read_dir(dir)?
            //TODO use filter_map
            .map(|r| r.map_err(crate::error::Error::from))
            .filter(|entry| {
                let entry = entry.as_ref().unwrap();
                let entry_path = entry.path();
                let filename = entry_path.file_name().unwrap();
                let file_name_as_str = filename.to_string_lossy();
                let not_gen_match = !re.is_match(&file_name_as_str);
                not_gen_match
            })
            .filter_map(Result::ok)
            .collect::<Vec<DirEntry>>();

        Ok(r)
    }
}

impl Storage for FileSystem<'_> {
    fn check_active_gen(&self) -> Result<u16> {
        let re = Regex::new(r#"^gen_(?P<gid>\d*)$"#).unwrap();

        let entries: Vec<u16> = std::fs::read_dir(self.base_path)?
            .filter_map(|read_dir| {
                let path = read_dir.as_ref().unwrap().path();
                if path.is_dir() {
                    path.file_name()
                        .map(|name| name.to_string_lossy().to_string())
                } else {
                    None
                }
            })
            .filter(|dir_name| dir_name.starts_with("gen_"))
            .filter_map(|dir_name| {
                let caps = re.captures(&dir_name).unwrap();
                caps["gid"].parse::<u16>().ok()
            })
            .collect();

        Ok(entries.into_iter().max().unwrap_or(1))
    }

    // fn retrieve_previous_gen<'de, P>(&self) -> Result<Generation<P>> where P: Deserialize<'de>;
    fn retrieve_previous_gen<P>(&self) -> Result<Generation<P>>
    where
        P: DeserializeOwned,
    {
        let prev_gen_no = self.check_active_gen().unwrap_or(1) - 1;
        let previous_gen_dir = self.base_path.join(format!("gen_{:03}", prev_gen_no));
        let file_path = previous_gen_dir.join(format!("gen_{:03}.json", prev_gen_no));
        let file = File::open(file_path)?;
        let reader = BufReader::new(file);

        //TODO why isn't our conversion in error.rs being applied?
        // let gen: Result<Generation<P>> = serde_json::from_reader(reader);

        let gen: Generation<P> = serde_json::from_reader(reader)?;

        Ok(gen)
    }

    fn save_particle<P: Serialize>(&self, w: &Particle<P>) -> Result<String> {
        let file_uuid = Uuid::new_v4();
        let file_path = self.base_path.join(file_uuid.to_string() + ".json");

        let pretty_json = serde_json::to_string_pretty(w);
        std::fs::write(&file_path, pretty_json?)?;

        Ok(file_path.to_string_lossy().into_owned())
    }

    fn num_particles_available(&self) -> Result<u16> {
        let files_in_folder = self.get_particle_files_in_current_gen_folder();

        match files_in_folder {
            Err(_) if self.check_active_gen().ok() == Some(1) => Ok(0),
            Ok(files) => Ok(files.len() as u16), //TODO read dir numbers & take max //TODO safer way to do cast - Ok(u16::try_from(file.len()))
            Err(e) => Err(e),
        }
    }

    fn retrieve_all_particles<P: DeserializeOwned>(&self) -> Result<Vec<Particle<P>>> {
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

    fn save_new_gen<P: Serialize>(&self, g: Generation<P>) -> Result<()> {
        let gen_dir = self
            .base_path
            .join(format!("gen_{:03}", g.generation_number));
        let file_path = gen_dir.join(format!("gen_{:03}.json", g.generation_number));

        match file_path.exists() {
            false => {
                let serialised_gen = serde_json::to_string_pretty(&g);
                std::fs::write(&file_path, serialised_gen?)?;
                Ok(())
            }
            true => Err(Error::GenAlreadySaved(format!(
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

    use super::*;

    struct TmpDir(PathBuf);
    impl TmpDir {
        pub fn new(name: &str) -> Self {
            let tmp_path = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("target")
                .join("test_tmp")
                .join(name);
            if tmp_path.exists() {
                std::fs::remove_dir_all(&tmp_path).unwrap();
            }
            std::fs::create_dir_all(&tmp_path).expect("failed to create");
            TmpDir(tmp_path)
        }
    }
    impl Drop for TmpDir {
        fn drop(&mut self) {
            // std::fs::remove_dir_all(self.0.as_path()).unwrap();
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

    fn storage(p: &Path) -> FileSystem {
        FileSystem { base_path: p }
    }

    fn make_dummy_generation(gen_number: u16) -> Generation<DummyParams> {
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

        Generation {
            generation_number: gen_number,
            tolerance: 0.1234,
            acceptance: 0.7,
            particles: vec![particle_1, particle_2],
        }
    }

    #[test]
    fn test_check_initial_active_gen() {
        let full_path = manifest_dir().join("resources/test/fs/empty");
        let storage = storage(&full_path);
        assert_eq!(1, storage.check_active_gen().unwrap());
    }

    #[test]
    fn test_check_active_gen() {
        let full_path = manifest_dir().join("resources/test/fs/example");
        let storage = storage(&full_path);
        assert_eq!(3, storage.check_active_gen().unwrap());
    }

    #[test]
    fn test_retrieve_previous_gen() {
        let expected = make_dummy_generation(2);

        let full_path = manifest_dir().join("resources/test/fs/example/");
        let instance = storage(&full_path);
        let result = instance.retrieve_previous_gen::<DummyParams>();
        let result = instance
            .retrieve_previous_gen::<DummyParams>()
            .expect(&format!("{:?}", result));

        assert_eq!(expected, result);
    }

    #[test]
    fn test_save_particle() {
        let tmp_dir = TmpDir::new("save_particle");
        let storage = storage(&tmp_dir.0);

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
        // println!("File was saved to {}", saved_1);
        let _saved_2 = storage.save_particle(&w2).unwrap();

        let file = std::fs::File::open(tmp_dir.0.clone().join(saved_1)).unwrap();
        // println!("About to try and load from {:?}", file);
        let loaded: Particle<DummyParams> =
            serde_json::from_reader(std::io::BufReader::new(file)).unwrap();

        assert_eq!(w1, loaded);
    }

    #[test]
    fn test_no_particle_files_initially() {
        let full_path = manifest_dir().join("resources/test/fs/empty/");
        let storage = storage(&full_path);
        assert_eq!(0, storage.num_particles_available().unwrap())
    }

    #[test]
    fn test_number_particle_files() {
        let full_path = manifest_dir().join("resources/test/fs/example/");
        let storage = storage(&full_path);
        assert_eq!(2, storage.num_particles_available().unwrap())
    }

    #[test]
    fn test_retrieve_particle_files() {
        let full_path = manifest_dir().join("resources/test/fs/example/");
        let instance = storage(&full_path);

        let mut expected /*: Result<Vec<Weighted<DummyParams>>>*/ = {    
            let w1 = Particle {
                parameters: DummyParams::new(1,2.),
                scores: vec![100.0, 200.0],
                weight: 1.234,
            };
    
            let w2 = Particle {
                parameters: DummyParams::new(3,4.),
                scores: vec![300.0, 400.0],
                weight: 1.567,
            };

            vec![w1, w2]
        };

        let mut result: Vec<Particle<DummyParams>> = instance.retrieve_all_particles().unwrap();

        //Sort by weight for easy comparison
        expected.sort_by(|a, b| a.weight.partial_cmp(&b.weight).unwrap());
        result.sort_by(|a, b| a.weight.partial_cmp(&b.weight).unwrap());

        assert_eq!(expected, result);
    }

    #[test]
    fn save_new_generation() {
        let tmp_dir = TmpDir::new("save_generation");
        let instance = storage(&tmp_dir.0);

        let gen = make_dummy_generation(3);
        std::fs::create_dir(instance.base_path.join("gen_003"))
            .expect("Expected successful dir creation");

        instance
            .save_new_gen(gen)
            .expect("Expected successful save");

        let expected = serde_json::json!({
            "generation_number": 3,
            "tolerance": 0.1234,
            "acceptance": 0.7,
            "particles": [
                {
                    "parameters" : {
                        "a": 10, "b": 20.0
                    },
                    "scores": [1000.0, 2000.0],
                    "weight": 0.234
                },{
                    "parameters" : {
                        "a": 30, "b": 40.0
                    },
                    "scores": [3000.0, 4000.0],
                    "weight": 0.567
                }
            ]
        });

        let actual = {
            let file = File::open(&tmp_dir.0.join("gen_003").join("gen_003.json")).unwrap();
            println!("Trying to load gen from {:?}", file);
            let reader = BufReader::new(file);
            serde_json::from_reader::<_, Value>(reader).unwrap()
        };

        assert_eq!(expected, actual);
    }

    #[test]
    fn dont_save_over_existing_gen_file() {
        let tmp_dir = TmpDir::new("save_over_generation");
        let instance = storage(&tmp_dir.0);

        //1. Save an dummy gen_003 file, representing file already save by another node
        std::fs::create_dir(instance.base_path.join("gen_003"))
            .expect("Expected successful dir creation");
        std::fs::write(
            tmp_dir.0.join("gen_003").join("gen_003.json"),
            "placeholder file",
        )
        .unwrap();

        //2. Try to save another gen over it, pretending we didn't notice the other node save gen before us
        let gen = make_dummy_generation(3);
        let result = instance.save_new_gen(gen);

        //3. Test that the original file save by other node is intact and we didn't panic.
        let contents =
            std::fs::read_to_string(tmp_dir.0.join("gen_003").join("gen_003.json")).unwrap();
        assert_eq!("placeholder file", contents);

        //4. Test that Result is Err::GenAlreadyExists()
        match result.unwrap_err() {
            Error::GenAlreadySaved(_) => (),
            other_error => panic!("Wrong error type: {}", other_error),
        };
    }
}
