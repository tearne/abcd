use futures::FutureExt;
use regex::Regex;
use rusoto_core::RusotoError;
use rusoto_s3::{
    GetObjectOutput, GetObjectRequest, ListObjectsV2Request, Object, PutObjectRequest, S3Client, S3,
};
use serde::{de::DeserializeOwned, Serialize};
use std::borrow::Borrow;
use std::convert::TryInto;
use std::fmt::Debug;
use tokio::runtime::Runtime;

use super::Storage;
use crate::error::{ABCDError, ABCDResult};
use crate::{Generation, Particle, Population};
use std::io::Read;
use tokio;
use uuid::Uuid;

pub struct S3System {
    bucket: String,
    prefix: String,
    s3_client: S3Client,
    runtime: Runtime,
}
impl S3System {
    fn get_particle_files_in_current_gen_folder(&self) -> ABCDResult<Vec<Object>> {
        //
        //Leaving this here for a bit to discuss some of the finer points with Tom
        //
        // let gen_no = self.check_active_gen().unwrap_or(1);
        // let gen_dir = format!("gen_{:03}", gen_no);
        // let prefix_cloned = self.prefix.clone();
        // let bucket_cloned = self.bucket.clone();
        // let gen_prefix = format!("{}/{}", prefix_cloned, gen_dir);

        // let list_request = self.s3_client
        //     .list_objects_v2(ListObjectsV2Request {
        //         bucket: String::from(bucket_cloned),
        //         prefix: Some(gen_prefix),
        //         ..Default::default()
        //     })
        //     .map(|response|{
        //         let response = response?;
        //         response.contents.ok_or_else(||ABCDError::Other("Empty response".into()))
        //     });

        // self.runtime.block_on(list_request)

        let gen_prefix = {
            let gen_no = self.check_active_gen().unwrap_or(1);
            let gen_dir = format!("gen_{:03}", gen_no);
            let prefix_cloned = self.prefix.clone();
            format!("{}/{}", prefix_cloned, gen_dir)
        };

        //TODO This is where we need to loop with continuation tokens
        let request = self.s3_client.list_objects_v2(ListObjectsV2Request {
            bucket: self.bucket.clone(),
            prefix: Some(gen_prefix),
            ..Default::default()
        });

        self.runtime
            .block_on(request)?
            .contents
            .ok_or_else(|| ABCDError::Other("Empty S3 response".into()))
    }

    pub async fn read_to_string<E: 'static + std::error::Error>(
        output: Result<GetObjectOutput, RusotoError<E>>,
    ) -> ABCDResult<String> {
        let byte_stream = output?
            .body
            .ok_or_else(|| ABCDError::Other("No body in S3 response.".into()))?;
        let mut string_buf: String = String::new();
        use tokio::io::AsyncReadExt;
        byte_stream
            .into_async_read()
            .read_to_string(&mut string_buf)
            .await?;
        Ok(string_buf)
    }
}
impl Storage for S3System {
    fn check_active_gen(&self) -> ABCDResult<u16> {
        let prefix_cloned = self.prefix.clone();
        let bucket_cloned = self.bucket.clone();

        let list_request_fut = self.s3_client.list_objects_v2(ListObjectsV2Request {
            bucket: String::from(bucket_cloned),
            prefix: Some(prefix_cloned),
            ..Default::default()
        });

        let objects = self
            .runtime
            .block_on(list_request_fut)?
            .contents
            .ok_or_else(|| ABCDError::Other("Empty S3 response body".into()))?;

        //TODO compile regex only once for entire struct.
        let re = Regex::new(r#"^example/gen_(?P<gid1>\d*)/gen_(?P<gid2>\d*).json"#)?;
        let key_strings = objects.into_iter().filter_map(|obj| obj.key);
        let gen_dir_numbers: Vec<u16> = key_strings
            .filter_map(|key| {
                re.captures(&key)
                    .map(|caps| caps["gid1"].parse::<u16>().ok())
                    .flatten()
            })
            .collect();

        let max_completed_gen = gen_dir_numbers.into_iter().max().unwrap_or(0);
        Ok(max_completed_gen + 1)

        //
        //Leaving this here for a bit to discuss some of the finer points with Tom
        //
        // let gen_number_future = list_request_fut.map(|response| {
        //     let contents = response.unwrap().contents.unwrap(); //TODO use ?

        //     let items = contents.iter();
        //     let key_strings = items.filter_map(|obj|obj.key.as_ref());

        //     let re = Regex::new(r#"^example/gen_(?P<gid1>\d*)/gen_(?P<gid2>\d*).json"#).unwrap(); //TODO use ?
        //     let gen_dir_numbers: Vec<u16> = key_strings
        //         .filter_map(|key|{
        //             re.captures(key)
        //                 .map(|caps|caps["gid1"].parse::<u16>().ok())
        //                 .flatten()
        //         })
        //         .collect();

        //     let max_completed_gen = gen_dir_numbers.into_iter().max().unwrap_or(0);
        //     Ok(max_completed_gen + 1)
        // });

        // self.runtime.block_on(gen_number_future)
    }

    fn retrieve_previous_gen<P>(&self) -> ABCDResult<Generation<P>>
    where
        P: DeserializeOwned + Debug,
    {
        let object_key = {
            let prev_gen_no = self.check_active_gen().unwrap_or(1) - 1;
            let prev_gen_file_dir = format!("gen_{:03}", prev_gen_no);
            let prev_gen_file_name = format!("gen_{:03}.json", prev_gen_no);

            format!(
                "{}/{}/{}",
                self.prefix.clone(),
                prev_gen_file_dir,
                prev_gen_file_name
            )
        };

        let get_obj_req = GetObjectRequest {
            bucket: self.bucket.clone(),
            key: object_key,
            ..Default::default()
        };

        let object_string_future = self
            .s3_client
            .get_object(get_obj_req)
            .then(Self::read_to_string);

        let string = self.runtime.block_on(object_string_future)?;
        let parsed: Generation<P> = serde_json::from_str(&string)?;
        Ok(parsed)
    }

    fn save_particle<P: Serialize>(&self, w: &Particle<P>) -> ABCDResult<String> {
        let s3_object_path = {
            let gen_no = self.check_active_gen().unwrap_or(1);
            let gen_file_dir = format!("gen_{:03}", gen_no);
            let file_uuid = Uuid::new_v4();
            let particle_file_name = file_uuid.to_string() + ".json";
            let prefix_cloned = self.prefix.clone();

            format!("{}/{}/{}", prefix_cloned, gen_file_dir, particle_file_name)
        };

        let pretty_json = serde_json::to_string_pretty(w)?;

        let put_obj_req = PutObjectRequest {
            bucket: self.bucket.clone(),
            key: s3_object_path.to_owned(),
            body: Some(pretty_json.to_owned().into_bytes().into()),
            acl: Some("bucket-owner-full-control".to_string()),
            ..Default::default()
        };

        let put_future = self.s3_client.put_object(put_obj_req);
        self.runtime.block_on(put_future)?;

        Ok(s3_object_path)
    }

    fn num_particles_available(&self) -> ABCDResult<u32> {
        let files_in_folder = self.get_particle_files_in_current_gen_folder();
        match files_in_folder {
            // Err(_) if self.check_active_gen().ok() == Some(1) => Ok(0),
            Ok(files) => Ok(files.len().try_into()?), //TODO read dir numbers & take max
            Err(e) => Err(e),
        }
    }

    fn retrieve_all_particles<P>(&self) -> ABCDResult<Vec<Particle<P>>>
    where
        P: DeserializeOwned,
    {
        let object_names = self
            .get_particle_files_in_current_gen_folder()?
            .into_iter()
            .map(|t| t.key)
            .collect::<Option<Vec<String>>>()
            .ok_or_else(|| ABCDError::Other("failed to identify all particle file names".into()))?;

        let particle_futures = object_names.into_iter().map(|filename| {
            let get_obj_req = GetObjectRequest {
                bucket: self.bucket.clone(),
                key: filename,
                ..Default::default()
            };
            self.s3_client
                .get_object(get_obj_req)
                .then(Self::read_to_string)
                .map(|res| {
                    res.and_then(|s| {
                        serde_json::from_str::<Particle<P>>(&s)
                            .map_err(|_| ABCDError::Other("badness".into()))
                    })
                })
        });

        let joined = futures::future::join_all(particle_futures);
        let particle_futures: Vec<Result<Particle<P>, ABCDError>> = self.runtime.block_on(joined);
        let result_of_vec: ABCDResult<Vec<Particle<P>>> = particle_futures.into_iter().collect();
        result_of_vec

        // let mut weighted_particles = Vec::new();
        // for entry in particle_files {
        //     let particle_filename = entry.key.unwrap();
        //     let get_obj_req = GetObjectRequest {
        //         bucket: self.bucket.clone(),
        //         key: particle_filename.to_owned(),
        //         ..Default::default()
        //     };

        //     let get_req = self.s3_client.get_object(get_obj_req);
        //     let mut response = self.runtime.block_on(get_req).unwrap();
        //     let stream = response.body.take().unwrap();
        //     // let t = stream.to_vec();
        //     let mut string: String = String::new();
        //     let _ = stream.into_blocking_read().read_to_string(&mut string);
        //     let wp: Particle<P> = serde_json::from_str(&string)?;
        //     weighted_particles.push(wp);
        // }

        // Ok(weighted_particles)
    }

    fn save_new_gen<P: Serialize>(
        &self,
        g: &Population<P>,
        generation_number: u16,
    ) -> ABCDResult<()> {
        //unimplemented!();
        let gen_dir = format!("gen_{:03}", generation_number);
        let file_name = format!("gen_{:03}.json", generation_number);
        let prefix_cloned = self.prefix.clone();
        let filename = format!("{}/{}/{}", prefix_cloned, gen_dir, file_name);
        let bucket_cloned = self.bucket.clone();
        let bucket_cloned2 = self.bucket.clone();

        let get_obj_req = GetObjectRequest {
            bucket: bucket_cloned,
            key: filename.to_owned(),
            ..Default::default()
        };
        //println!("{:?}", &get_obj_req);
        let get_req = self.s3_client.get_object(get_obj_req);
        let mut response = self.runtime.block_on(get_req);

        match response.is_err() {
            //Is there something there already - if there is an error then there isn't?
            true => {
                let pretty_json_gen = serde_json::to_string_pretty(&g);
                let put_obj_req = PutObjectRequest {
                    bucket: bucket_cloned2,
                    key: filename,
                    body: Some(pretty_json_gen.unwrap().into_bytes().into()),
                    acl: Some("bucket-owner-full-control".to_string()),
                    ..Default::default()
                };
                let put_req = self.s3_client.put_object(put_obj_req);
                self.runtime.block_on(put_req)?;
                Ok(())
            }
            false => Err(ABCDError::GenAlreadySaved(format!(
                "Gen file already existed at {:?}",
                filename
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use rusoto_core::Region;
    use rusoto_s3::DeleteObjectRequest;
    use serde::{Deserialize, Serialize};
    use serde_json::Value;
    use std::io::Read;

    use crate::{etc::config::Config, storage::config::StorageConfig};

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
            println!("{:?}", &bucket_prefix_contents);

            if !bucket_prefix_contents.is_empty() {
                let items = bucket_prefix_contents.iter();
                let keys_to_delete = items.map(|key| {
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
        //Override prefix in config to use the temp one made for this test
        let storage_config = Config::from_path("resources/test/config_test.toml").storage;
        let storage_config = StorageConfig::S3 {
            prefix,
            bucket: storage_config.get_bucket().into(),
        };

        storage_config.build_s3()
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

    fn load_object(bucket: &str, key: &str) -> String {
        //     let gen_file_dir = format!("gen_{:03}", gen_number);
        //     let gen_file_name = format!("gen_{:03}.json", gen_number);
        //     let s3_client = S3Client::new(Region::EuWest1);
        //     let storage = storage(
        //          prefix.to_string(),
        //         s3_client,
        //     );
        //     // let separator = "/".to_string();
        //     let prefix_cloned = storage.prefix.clone();
        //     let filename = format!(
        //         "{}/{}/{}",
        //         prefix_cloned, gen_file_dir, gen_file_name
        //     );
        //     let bucket_cloned = storage.bucket.clone();
        //     println!("Requesting {}", filename);
        // let get_req = GetObjectRequest {
        //     bucket: bucket.into(),
        //     key: key.into(),
        //     ..Default::default()
        // };
        // println!("{:?}", &get_obj_req);

        let s3_client = S3Client::new(Region::EuWest1);
        let request = s3_client
            .get_object(GetObjectRequest {
                bucket: bucket.into(),
                key: key.into(),
                ..Default::default()
            })
            .then(S3System::read_to_string);
        tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(request)
            .unwrap()
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
        let gen_number = 3;
        let expected = Generation::Population {
            pop: make_dummy_population(),
            gen_number,
        };
        let s3_client = S3Client::new(Region::EuWest1);
        //TODO call make storage
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

        let mut result: Vec<Particle<DummyParams>> = storage.retrieve_all_particles().unwrap();

        //Sort by weight for easy comparison
        expected.sort_by(|a, b| a.weight.partial_cmp(&b.weight).unwrap());
        result.sort_by(|a, b| a.weight.partial_cmp(&b.weight).unwrap());

        assert_eq!(expected, result);
    }

    #[test]
    fn save_and_load_generation() {
        let gen_number = 3;
        let dummy_population = make_dummy_population();

        let s3_client = S3Client::new(Region::EuWest1);
        let tmp_bucket = TmpBucketPrefix::new("save_generation"); //Clear bucket if anything there
        let storage = storage("save_generation".to_string(), s3_client);
        storage
            .save_new_gen(&dummy_population, 3)
            .expect("Expected successful save");

        let expected: Value = serde_json::to_value(Generation::Population {
            pop: dummy_population,
            gen_number,
        })
        .unwrap();

        let actual: Value = serde_json::from_str(&load_object(
            &storage.bucket,
            &format!("gen_{:03}/gen_{:03}.json", gen_number, gen_number),
        ))
        .unwrap();

        assert_eq!(expected, actual);
    }

    // #[test]
    // fn dont_save_over_existing_gen_file(){
    //     let expected = make_dummy_generation(3);
    //     let s3_client = S3Client::new(Region::EuWest1);
    //     let tmp_bucket = TmpBucketPrefix::new("save_generation"); //Clear bucket if anything there
    //     let storage = storage("save_generation".to_string(), s3_client);

    //     //1. Save an dummy gen_003 file, representing file already save by another node
    //     storage.save_new_gen(make_dummy_generation(3)).expect("Expected successful save");

    //     //2. Try to save another gen over it, pretending we didn't notice the other node save gen before us
    //     let result = storage.save_new_gen(make_dummy_generation(3)).expect("Expected successful save");

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
