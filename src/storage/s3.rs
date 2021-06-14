use futures::FutureExt;
use rusoto_s3::{ListObjectsV2Request, Object, S3, S3Client,GetObjectRequest};
use rusoto_core::Region;
use serde::{Serialize, de::DeserializeOwned};
use tokio::fs::read_to_string;
use tokio::runtime::Runtime;
use regex::Regex;
use std::fmt::Debug;

use crate::{Generation, Particle};
use crate::error::{Error, Result};
use super::Storage;
use tokio;
use std::convert::TryInto;
use std::fs::File;
use std::io::BufReader;

struct S3System {
    bucket:  String,
    prefix:  String,
    s3_client:  S3Client,
    runtime: Runtime,
}
impl Storage for S3System {
    fn check_active_gen(&self) -> Result<u16> {
        let prefix_cloned = self.prefix.clone();
        let bucket_cloned = self.bucket.clone();

        let list_request_fut = self.s3_client.list_objects_v2(ListObjectsV2Request{
            bucket: String::from(bucket_cloned),
            prefix: Some(prefix_cloned),
            ..Default::default()
        });

        let gen_number_fut = list_request_fut.map(|response| {
            let contents = response.unwrap().contents.unwrap();
            let re = Regex::new(r#"^example/gen_(?P<gid1>\d*)/gen_(?P<gid2>\d*).json"#).unwrap(); //TODO use ?
            let items = contents.iter();
            let keys: Vec<u16> = items
                .filter_map(|key| {
                    let key_string = key.key.as_ref().unwrap();
                    let dir_match = re.is_match(&key_string);
                    let dir_no = match dir_match {
                        true => re.captures(&key_string).unwrap()["gid1"].parse::<u16>().ok(),
                        false => None
                    };
                    //println!("captures = {:?}", caps["gid2"].parse::<u16>());
                    println!("dir_no = {:?}", dir_no);
                    dir_no  
                })
               .collect();
            
            let max_finished_gen = keys.into_iter().max().unwrap_or(0);
            max_finished_gen
        });

        let answer = self.runtime.block_on(gen_number_fut);
        Ok(answer+1) //Last gen with a gen file +1
    }

    fn retrieve_previous_gen<'de, P>(&self) -> Result<Generation<P>> where P: DeserializeOwned + Debug {
        let prev_gen_no = self.check_active_gen().unwrap_or(1) -1;
        let prev_gen_file_dir = format!("gen_{:03}", prev_gen_no);
        let prev_gen_file_name = format!("gen_{:03}.json", prev_gen_no);
        // let separator = "/".to_string();
        let prefix_cloned = self.prefix.clone();
        let filename =  format!("{}/{}/{}", prefix_cloned,prev_gen_file_dir,prev_gen_file_name);
        let bucket_cloned = self.bucket.clone();

        println!("Requesting {}", filename);


        let get_obj_req = GetObjectRequest 
        { 
            bucket: bucket_cloned,
            key:filename.to_owned(),
            ..Default::default()
        };
        println!("{:?}",&get_obj_req);
        let get_req = self.s3_client.get_object(get_obj_req);

        

        let mut response = self.runtime.block_on(get_req).unwrap();



        let stream = response.body.take().unwrap();
        // let t = stream.to_vec();
        use std::io::Read;
        let mut string: String = String::new();
        let _ = stream.into_blocking_read().read_to_string(&mut string);
        println!(" ========> {}", string);

        let parsed: Generation<P> = serde_json::from_str(&string)?;

        println!("Parsed to {:?}", parsed);

        Ok(parsed)
        
        // let string = String::from_utf8_lossy(stream);
        
        // use tokio::io::AsyncReadExt;
        // use tokio::fs::By
        // stream.read_to_end(String::new());




        // let mut body = stream.into_async_read();
       // let body = body.map_ok(|b| b.to_vec()).


    //     let gen: Generation<P> = serde_json::from_reader(reader)?;

    //     Ok(gen)
    }
    fn save_particle<P: Serialize>(&self, w: &Particle<P>) -> Result<String>{
        unimplemented!();
    }
    fn num_particles_available(&self) -> Result<u16>{
        unimplemented!();
    }
    fn retrieve_all_particles<P>(&self) -> Result<Vec<Particle<P>>> where P: DeserializeOwned {
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
        let runtime = Runtime::new().unwrap();

        S3System{
            bucket,
            prefix,
            s3_client,
            runtime,
        }
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
        let storage = storage("s3-ranch-007".to_string(),"example/".to_string(),s3_client);
        
        assert_eq!(3, storage.check_active_gen().unwrap());
    }

    #[test]
    fn test_retrieve_previous_gen() {
        let expected = make_dummy_generation(2);
        let s3_client = S3Client::new(Region::EuWest1);
        let storage = storage("s3-ranch-007".to_string(),"example".to_string(),s3_client);
        let result = storage.retrieve_previous_gen::<DummyParams>();
        let result = 
            storage.retrieve_previous_gen::<DummyParams>().expect(&format!("{:?}", result));

        assert_eq!(expected, result);
    }
}