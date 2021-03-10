use std::path::Path;

use regex::Regex;
use serde::Serialize;

use crate::{Generation, Weighted};
use crate::error::Result;
use uuid::Uuid;
use std::convert::TryInto;

trait Storage {
    fn check_active_gen(&self) -> Result<u16>;
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
    fn check_active_gen(&self) -> Result<u16> {
        let re = Regex::new(r#"^gen_(?P<gid>\d*)$"#).unwrap();

        let entries: Vec<u16> = std::fs::read_dir(self.base_path)?
            .filter_map(|read_dir|{
                let path = read_dir.as_ref().unwrap().path();
                if path.is_dir() {
                    path.file_name().map(|name| name.to_string_lossy().to_string())
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

        Ok(entries.into_iter().max().unwrap_or(0))  //TODO does it make sense to return 0 if no max?
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
        //TODO test case for when returns 1?
        let gen_no = self.check_active_gen().unwrap_or(1);
        let gen_dir = format!("gen_{:03}.json", gen_no);
        let dir = self.base_path.join(gen_dir);

        let re = Regex::new(r#"^gen_(?P<gid>\d*)$"#).unwrap(); //TODO use ?
        
        let particle_files :Vec<_> = std::fs::read_dir(dir)?
            .filter(|entry| {
                let entry = entry.as_ref().unwrap();
                let entry_path = entry.path();
                let filename = entry_path.file_name().unwrap();
                let file_name_as_str = filename.to_string_lossy();
                let not_gen_match = !re.is_match(&file_name_as_str);
                not_gen_match
            }).collect();
        
        Ok(particle_files.len().try_into().unwrap()) //TODO what's all this about then?
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
    use std::path::{Path, PathBuf};
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
        let full_path = manifest_dir().join("resources/test/fs/empty/");
        let storage = storage(&full_path);
        assert_eq!(0, storage.check_active_gen().unwrap());
    }

    #[test]
    fn test_check_active_gen() {
        let full_path = manifest_dir().join("resources/test/fs/example");
        let storage = storage(&full_path);
        assert_eq!(3, storage.check_active_gen().unwrap());
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
        let full_path = manifest_dir().join("resources/test/fs/empty/");
        let storage = storage(&full_path);
        assert_eq!(0,storage.num_particles_available().unwrap())
    }


    #[test]
    fn test_number_particle_files() {
        let full_path = manifest_dir().join("resources/test/fs/example/");
        let storage = storage(&full_path);
        assert_eq!(1,storage.num_particles_available().unwrap())
    }


}