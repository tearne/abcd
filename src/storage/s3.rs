use futures::{Future, FutureExt, TryFutureExt};
use regex::Regex;
use rusoto_core::Region;
use rusoto_s3::{
    DeleteObjectRequest, GetObjectRequest, ListObjectsV2Request, Object, PutObjectRequest,
    S3Client, S3,
};
use serde::{de::DeserializeOwned, Serialize};
use std::env;
use std::fmt::Debug;
use tokio::fs::read_to_string;
use tokio::runtime::Runtime; 
//For environment variables => https://doc.rust-lang.org/book/ch12-05-working-with-environment-variables.html

use super::Storage;
use crate::error::{Error, Result};
use crate::{Config, Generation, Particle};
use std::convert::TryInto;
use std::fs::File;
use std::io::BufReader;
use std::io::Read;
use tokio;
use uuid::Uuid;

struct S3System {
    bucket: String,
    prefix: String,
    s3_client: S3Client,
    runtime: Runtime,
}
impl S3System {
    fn get_particle_files_in_current_gen_folder(&self) -> Result<Vec<Object>> {
        //TODO This is where we want to loop more than 1000
        let gen_no = self.check_active_gen().unwrap_or(1);
        let gen_dir = format!("gen_{:03}", gen_no);
        let prefix_cloned = self.prefix.clone();
        let bucket_cloned = self.bucket.clone();
        let gen_prefix = format!("{}/{}", prefix_cloned, gen_dir);

        //println!("Requesting {}", gen_prefix);

        let list_request_fut = self.s3_client.list_objects_v2(ListObjectsV2Request {
            bucket: String::from(bucket_cloned),
            prefix: Some(gen_prefix),
            ..Default::default()
        });

        let current_gen_fut = list_request_fut.map(|response| {
            let contents = response.unwrap().contents.unwrap();
            //  println!("Contents ====> {:?}",&contents);
            contents
        });
        let answer = self.runtime.block_on(current_gen_fut);
        Ok(answer)
    }
}
impl Storage for S3System {
    fn check_active_gen(&self) -> Result<u16> {
        let prefix_cloned = self.prefix.clone();
        let bucket_cloned = self.bucket.clone();

        let list_request_fut = self.s3_client.list_objects_v2(ListObjectsV2Request {
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
                        true => re.captures(&key_string).unwrap()["gid1"]
                            .parse::<u16>()
                            .ok(),
                        false => None,
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
        Ok(answer + 1) //Last gen with a gen file +1
    }

    fn retrieve_previous_gen<'de, P>(&self) -> Result<Generation<P>>
    where
        P: DeserializeOwned + Debug,
    {
        let prev_gen_no = self.check_active_gen().unwrap_or(1) - 1;
        let prev_gen_file_dir = format!("gen_{:03}", prev_gen_no);
        let prev_gen_file_name = format!("gen_{:03}.json", prev_gen_no);
        // let separator = "/".to_string();
        let prefix_cloned = self.prefix.clone();
        let filename = format!(
            "{}/{}/{}",
            prefix_cloned, prev_gen_file_dir, prev_gen_file_name
        );
        let bucket_cloned = self.bucket.clone();
        println!("Requesting {}", filename);
        let get_obj_req = GetObjectRequest {
            bucket: bucket_cloned,
            key: filename.to_owned(),
            ..Default::default()
        };
        println!("{:?}", &get_obj_req);
        let get_req = self.s3_client.get_object(get_obj_req);

        let string_fut = get_req.then(move |gor| async {
            let mut gor = gor.unwrap();
            let stream = gor.body.take().unwrap();
            use tokio::io::AsyncReadExt;
            let mut string_buf: String = String::new();
            let outcome = stream
                .into_async_read()
                .read_to_string(&mut string_buf)
                .await;
            println!("Async read result = {:#?}", outcome);
            string_buf
        });

        let string = self.runtime.block_on(string_fut);
        let parsed: Generation<P> = serde_json::from_str(&string)?;
        println!("Parsed to {:?}", parsed);
        Ok(parsed)
    }

    fn save_particle<P: Serialize>(&self, w: &Particle<P>) -> Result<String> {
        let gen_no = self.check_active_gen().unwrap_or(1);
        let gen_file_dir = format!("gen_{:03}", gen_no);
        let file_uuid = Uuid::new_v4();
        let particle_file_name = file_uuid.to_string() + ".json";
        let prefix_cloned = self.prefix.clone();
        let s3_file_path = format!("{}/{}/{}", prefix_cloned, gen_file_dir, particle_file_name);
        let bucket_cloned = self.bucket.clone();
        let pretty_json = serde_json::to_string_pretty(w);
        let put_obj_req = PutObjectRequest {
            bucket: bucket_cloned,
            key: s3_file_path.to_owned(),
            body: Some(pretty_json.unwrap().to_owned().into_bytes().into()),
            acl: Some("bucket-owner-full-control".to_string()),
            ..Default::default()
        };
        let put_req = self.s3_client.put_object(put_obj_req);
        let mut response = self.runtime.block_on(put_req).unwrap();

        Ok(s3_file_path)
        //Ok(particle_file_name)
    }
    fn num_particles_available(&self) -> Result<u16> {
        //unimplemented!();
        let files_in_folder = self.get_particle_files_in_current_gen_folder();
        match files_in_folder {
            Err(_) if self.check_active_gen().ok() == Some(1) => Ok(0),
            Ok(files) => Ok(files.len() as u16), //TODO read dir numbers & take max //TODO safer way to do cast - Ok(u16::try_from(file.len()))
            Err(e) => Err(e),
        }
    }

    fn retrieve_all_particles<P>(&self) -> Result<Vec<Particle<P>>>
    where
        P: DeserializeOwned,
    {
        // unimplemented!();
        let particle_files = self.get_particle_files_in_current_gen_folder()?;
        let bucket_cloned = self.bucket.clone();
        let mut weighted_particles = Vec::new();
        for entry in particle_files {
            let particle_filename = entry.key.unwrap();
            let get_obj_req = GetObjectRequest {
                bucket: bucket_cloned.to_owned(),
                key: particle_filename.to_owned(),
                ..Default::default()
            };
            //println!("{:?}",&get_obj_req);
            let get_req = self.s3_client.get_object(get_obj_req);
            let mut response = self.runtime.block_on(get_req).unwrap();
            let stream = response.body.take().unwrap();
            // let t = stream.to_vec();
            let mut string: String = String::new();
            let _ = stream.into_blocking_read().read_to_string(&mut string);
            let wp: Particle<P> = serde_json::from_str(&string)?;
            weighted_particles.push(wp);
        }

        Ok(weighted_particles)
    }

    fn save_new_gen<P: Serialize>(&self, g: Generation<P>) -> Result<()> {
        //unimplemented!();
        let gen_dir = format!("gen_{:03}", g.generation_number);
        let file_name = format!("gen_{:03}.json", g.generation_number);
        let prefix_cloned = self.prefix.clone();
        let filename = format!("{}/{}/{}", prefix_cloned, gen_dir, file_name);
        let bucket_cloned = self.bucket.clone();
        let bucket_cloned2 = self.bucket.clone();

        let get_obj_req = GetObjectRequest {
            bucket: bucket_cloned,
            key: filename.to_owned(),
            ..Default::default()
        };
        println!("{:?}", &get_obj_req);
        let get_req = self.s3_client.get_object(get_obj_req);
        let mut response = self.runtime.block_on(get_req).unwrap();

        match response.body.is_some() {
            //Is there something there already?
            false => {
                let pretty_json_gen = serde_json::to_string_pretty(&g);
                let put_obj_req = PutObjectRequest {
                    bucket: bucket_cloned2,
                    key: filename.to_owned(),
                    body: Some(pretty_json_gen.unwrap().to_owned().into_bytes().into()),
                    acl: Some("bucket-owner-full-control".to_string()),
                    ..Default::default()
                };
                let put_req = self.s3_client.put_object(put_obj_req);
                let mut response2 = self.runtime.block_on(put_req).unwrap();
                Ok(())
            }
            true => Err(Error::GenAlreadySaved(format!(
                "Gen file already existed at {:?}",
                filename
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use serde::{Deserialize, Serialize};
    use serde_json::Value;
    use std::{
        io::Read,
        path::{Path, PathBuf},
    };

    use super::*;

    struct TmpBucketPrefix(String);
    impl TmpBucketPrefix {
        pub fn new(prefix: &str) -> Self {
            let s3_client = S3Client::new(Region::EuWest1);
            let storage = storage(prefix.to_string(), s3_client);
            let prefix_cloned = storage.prefix.clone();
            let bucket_cloned = storage.bucket.clone();

            let list_request_fut = storage.s3_client.list_objects_v2(ListObjectsV2Request {
                bucket: String::from(bucket_cloned),
                prefix: Some(prefix_cloned),
                ..Default::default()
            });

            let fut = list_request_fut.map(|response| {
                let contents = response.unwrap().contents.unwrap();
                contents
            });
            let bucket_prefix_contents = storage.runtime.block_on(fut);

            if !bucket_prefix_contents.is_empty() {
                let items = bucket_prefix_contents.iter();
                let keys = items.map(|key| {
                    let delete_object_req = DeleteObjectRequest {
                        bucket: storage.bucket.to_owned(),
                        key: key.key.clone().unwrap(),
                        ..Default::default()
                    };
                    let req = storage.s3_client.delete_object(delete_object_req);
                });
            }
            TmpBucketPrefix(prefix.to_string())
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

    //fn storage(bucket:String,prefix:String,s3_client:S3Client) -> S3System {
    fn storage(prefix: String, s3_client: S3Client) -> S3System {
        // if !envmnt::exists("TEST_BUCKET") {
        //     envmnt::set("TEST_BUCKET", "testBucket");
        // } //Q is this only something related to simplelogger?
        //let config = Config::from_path(opt.config); // Leaving for now as can't read in env variables in toml file

        // TODO make this use the new config
        let bucket = env::var("TEST_BUCKET").unwrap().to_string(); //.expect("TEST_BUCKET not set");
        println!(" ====> bucket {}", bucket);

        let runtime = Runtime::new().unwrap();

        S3System {
            bucket,
            prefix,
            s3_client,
            runtime,
        }
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

    fn load_particle_file(particle_file_name: String) -> Particle<DummyParams> {
        let s3_client = S3Client::new(Region::EuWest1);
        //let storage = storage("s3-ranch-007".to_string(),"save_particle".to_string(),s3_client);
        let storage = storage(
            /*"s3-ranch-007".to_string(),*/ "example".to_string(),
            s3_client,
        );
        //let particle_file_dir = storage.prefix.clone();
        //let filename =  format!("{}/{}", particle_file_dir,particle_file_name);
        let bucket_cloned = storage.bucket.clone();
        let get_obj_req = GetObjectRequest {
            bucket: bucket_cloned,
            key: particle_file_name.to_owned(),
            ..Default::default()
        };
        let get_req = storage.s3_client.get_object(get_obj_req);
        let mut response = storage.runtime.block_on(get_req).unwrap();
        let stream = response.body.take().unwrap();
        let mut string: String = String::new();
        let _ = stream.into_blocking_read().read_to_string(&mut string);
        println!(" ========> {}", string);
        let parsed: Particle<DummyParams> = serde_json::from_str(&string).unwrap();
        parsed
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
        let storage = storage(
            /*"s3-ranch-007".to_string(),*/ "example/".to_string(),
            s3_client,
        );

        assert_eq!(3, storage.check_active_gen().unwrap());
    }

    #[test]
    fn test_retrieve_previous_gen() {
        let expected = make_dummy_generation(2);
        let s3_client = S3Client::new(Region::EuWest1);
        let storage = storage("example".to_string(), s3_client);
        let result = storage.retrieve_previous_gen::<DummyParams>();
        let result = storage
            .retrieve_previous_gen::<DummyParams>()
            .expect(&format!("{:?}", result));

        assert_eq!(expected, result);
    }

    #[test]
    fn test_save_particle() {
        let s3_client = S3Client::new(Region::EuWest1);
        let tmp_bucket = TmpBucketPrefix::new("save_particle");
        println!(
            "==============================> tmp_bucket {}",
            tmp_bucket.0
        );
        let storage = storage("save_particle".to_string(), s3_client);

        let p1 = DummyParams::new(1, 2.);
        let w1 = Particle {
            parameters: p1,
            scores: vec![100.0, 200.0],
            weight: 1.234,
        };

        // TODO fix the async problem like this:
        // https://github.com/hyperium/hyper/issues/2112
        // or rusoto you can create a new client https://docs.rs/rusoto_core/0.43.0/rusoto_core/request/struct.HttpClient.html 
        // from here and then pass that into the specific service's constructor. This will avoid using the lazy_static client.
        let saved_1 = storage.save_particle(&w1).unwrap();
        let loaded: Particle<DummyParams> = load_particle_file(saved_1);

        assert_eq!(w1, loaded);
        //If possible delete file that has just been saved - as it screws up later number of particles test - maybe implement temp dir in bucket
    }

    // #[test]
    // fn test_no_particle_files_initially() {
    //     let full_path = manifest_dir().join("resources/test/fs/empty/");
    //     let storage = storage(&full_path);
    //     assert_eq!(0,storage.num_particles_available().unwrap())
    // }

    #[test]
    fn test_number_particle_files() {
        let s3_client = S3Client::new(Region::EuWest1);
        let storage = storage("example".to_string(), s3_client);
        assert_eq!(2, storage.num_particles_available().unwrap())
    }

    #[test]
    fn test_retrieve_particle_files() {
        let s3_client = S3Client::new(Region::EuWest1);
        let storage = storage("example".to_string(), s3_client);

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

        let mut result: Vec<Particle<DummyParams>> = storage.retrieve_all_particles().unwrap();

        //Sort by weight for easy comparison
        expected.sort_by(|a, b| a.weight.partial_cmp(&b.weight).unwrap());
        result.sort_by(|a, b| a.weight.partial_cmp(&b.weight).unwrap());

        assert_eq!(expected, result);
    }

    // #[test]
    // fn save_new_generation(){
    //     // let tmp_dir = TmpDir::new("save_generation");
    //     // let instance = storage(&tmp_dir.0);

    //     //Again need to make possible temp s3 dir?

    //     let s3_client = S3Client::new(Region::EuWest1);
    //     let storage = storage("s3-ranch-007".to_string(),"example".to_string(),s3_client);

    //     let gen = make_dummy_generation(3);
    //     instance.save_new_gen(gen).expect("Expected successful save");

    //     let expected = serde_json::json!({
    //         "generation_number": 3,
    //         "tolerance": 0.1234,
    //         "acceptance": 0.7,
    //         "particles": [
    //             {
    //                 "parameters" : {
    //                     "a": 10, "b": 20.0
    //                 },
    //                 "scores": [1000.0, 2000.0],
    //                 "weight": 0.234
    //             },{
    //                 "parameters" : {
    //                     "a": 30, "b": 40.0
    //                 },
    //                 "scores": [3000.0, 4000.0],
    //                 "weight": 0.567
    //             }
    //         ]
    //     });

    //     let actual = {

    //     };

    //     assert_eq!(expected, actual);
    // }

    // #[test]
    // fn dont_save_over_existing_gen_file(){
    //     let tmp_dir = TmpDir::new("save_over_generation");
    //     let instance = storage(&tmp_dir.0);

    //     //1. Save an dummy gen_003 file, representing file already save by another node
    //     std::fs::create_dir(instance.base_path.join("gen_003")).expect("Expected successful dir creation");
    //     std::fs::write(tmp_dir.0.join("gen_003").join("gen_003.json"), "placeholder file").unwrap();

    //     //2. Try to save another gen over it, pretending we didn't notice the other node save gen before us
    //     let gen = make_dummy_generation(3);
    //     let result = instance.save_new_gen(gen);

    //     //3. Test that the original file save by other node is intact and we didn't panic.
    //     let contents = std::fs::read_to_string(tmp_dir.0.join("gen_003").join("gen_003.json")).unwrap();
    //     assert_eq!("placeholder file", contents);

    //     //4. Test that Result is Err::GenAlreadyExists()
    //     match result.unwrap_err(){
    //         Error::GenAlreadySaved(_) => (),
    //         other_error => panic!("Wrong error type: {}", other_error),
    //     };
    // }
}
