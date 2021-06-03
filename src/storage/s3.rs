use rusoto_s3::{ListObjectsV2Request, S3, S3Client};
use rusoto_core::Region;
use serde::{Serialize, de::DeserializeOwned};

use crate::{Generation, Particle};
use crate::error::{Error, Result};
use super::Storage;
use tokio;
use std::convert::TryInto;

struct S3System<'a> {
    bucket:  &'a String,
    prefix: &'a String,
    s3Client: &'a S3Client
}
impl Storage for S3System<'_> {
    fn check_active_gen(&self) -> Result<u16> {
        let mut key_names:Vec<String> = Vec::new();

        tokio::spawn( async move {
        let fut = self.s3Client.clone().list_objects_v2(ListObjectsV2Request{
            bucket: String::from(self.bucket.clone()),
            prefix: Some(self.prefix.clone()),
            ..Default::default()
        });
    
        let response = fut.await.unwrap();
    
        for keys in response.contents.unwrap() {
            let keyStr = keys.key.unwrap();
            if keyStr.starts_with("gen_") {
            key_names.push(keys.key.unwrap());
            }
        }
    });
    Ok(key_names.len().try_into().unwrap())
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

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};
    use serde::{Serialize, Deserialize};
    use serde_json::Value;

    use super::*;

/*     struct TmpDir(PathBuf);
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
            // std::fs::remove_dir_all(self.0.as_path()).unwrap();
        }
    } */

    #[derive(Serialize, Deserialize, Debug, PartialEq)]
    struct DummyParams{
        a: u16,
        b: f32
    }
    impl DummyParams{
        pub fn new(a: u16, b: f32) -> Self { DummyParams{a,b} }
    }

    fn storage(bucket:String,prefix:String,s3_client:S3Client) -> S3System {
        S3System{bucket:bucket,prefix:prefix,s3Client:s3Client}
    }

    fn make_dummy_generation(gen_number: u16) -> Generation<DummyParams> {
        let particle_1 = Particle {
            parameters: DummyParams::new(10,20.),
            scores: vec![1000.0, 2000.0],
            weight: 0.234,
        };

        let particle_2 = Particle {
            parameters: DummyParams::new(30,40.),
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

    // #[test]
    // fn test_check_initial_active_gen() {
    //     let full_path = manifest_dir().join("resources/test/fs/empty");
    //     let storage = storage(&full_path);
    //     assert_eq!(1, storage.check_active_gen().unwrap());
    // }

    #[test]
    fn test_check_active_gen() {
        let s3_client = S3Client::new(Region::EuWest1);
        let storage = storage("s3-ranch-007".to_string(),"example/gen_002/".to_string(),s3_client);
        
        assert_eq!(3, storage.check_active_gen().unwrap());
    }

  

}