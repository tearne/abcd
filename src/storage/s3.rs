use rusoto_s3::{ListObjectsV2Request, S3, S3Client};
use rusoto_core::Region;
use serde::{Serialize, de::DeserializeOwned};
use tokio::runtime::Runtime;

use crate::{Generation, Particle};
use crate::error::{Error, Result};
use super::Storage;
use tokio;
use std::convert::TryInto;

struct S3System {
    bucket:  String,
    prefix:  String,
    s3Client:  S3Client
}
impl Storage for S3System {
    fn check_active_gen(&self) -> Result<u16> {
        
        let cloned = self.s3Client.clone();
        let prefix_cloned = self.prefix.clone();
        let bucket_cloned = self.bucket.clone();

        let rt  = Runtime::new()?;

        

        let result = rt.block_on( async move {
            let fut = cloned.list_objects_v2(ListObjectsV2Request{
                bucket: String::from(bucket_cloned),
                prefix: Some(prefix_cloned),
                ..Default::default()
            });
        
            let response = fut.await.unwrap();
        
            let mut key_names:Vec<String> = Vec::new();
            for keys in response.contents.unwrap() {
                println!("{:#?}", keys.clone().key.unwrap());
                let key_str = keys.clone().key.unwrap();
                if key_str.starts_with("gen_") {
                    key_names.push(keys.clone().key.unwrap());
                }
            }

            key_names
        });

        // let t = Runtime::new().unwrap().block_on(result);
        
        Ok(result.len().try_into().unwrap())
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
        S3System{bucket:bucket,prefix:prefix,s3Client:s3_client}
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

    // #[test
    #[test]
    fn test_check_active_gen() {
        let s3_client = S3Client::new(Region::EuWest1);
        let storage = storage("s3-ranch-007".to_string(),"example/gen_002/".to_string(),s3_client);
        
        assert_eq!(3, storage.check_active_gen().unwrap());
    }

  

}