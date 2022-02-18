use aws_sdk_s3::error::GetObjectAclError;
use aws_sdk_s3::{Client, SdkError, ByteStream, Region};
use aws_sdk_s3::model::{Object, ObjectCannedAcl};
use aws_sdk_s3::output::{GetObjectOutput, PutObjectOutput, ListObjectsV2Output};
use bytes::Bytes;
use futures::{FutureExt, Future, TryFutureExt};
use regex::Regex;
use serde::{de::DeserializeOwned, Serialize};
use std::convert::TryInto;
use std::fmt::Debug;
use tokio::runtime::Runtime;

use super::Storage;
use crate::error::{ABCDError, ABCDResult};
use crate::{Generation, Particle};
use tokio;
use uuid::Uuid;

pub struct S3System {
    pub bucket: String,
    pub prefix: String,
    client: Client,
    runtime: Runtime,
}
impl S3System {
    pub fn new(bucket: String, prefix: String) -> Self {
        let runtime = Runtime::new().unwrap();
        let client = {
            let config = runtime.block_on(
                aws_config::from_env().region(Region::new("eu-west-1")).load()
            );
            Client::new(&config)
        };

        S3System {
            bucket,
            prefix,
            client,
            runtime,
        }
    }

    fn list_objects_v2(&self, prefix: &str) -> ABCDResult<Vec<Object>> {
        let mut acc: Vec<Object> = Vec::new();

        let next_page = |c_tok: Option<String>| -> ABCDResult<ListObjectsV2Output> {
            let request = self.client
                .list_objects_v2()
                .bucket(&self.bucket)
                .prefix(prefix)
                .set_continuation_token(c_tok)
                .send();

            self.runtime
                .block_on(request)
                .map_err(|e|e.into())
        };

        let mut c_token = None;
        loop {
            let list_output = next_page(c_token)?;
            if let Some(mut items) = list_output.contents {
                acc.append(&mut items);
            }
            
            c_token = list_output.continuation_token;
            if c_token.is_none() { break; }
        }

        Ok(acc)
    }

    fn get_particle_files_in_active_gen(&self) -> ABCDResult<Vec<Object>> {
        let gen_prefix = {
            let gen_no = self.previous_gen_number()?; //TODO test me
            let gen_dir = format!("gen_{:03}", gen_no);
            format!("{}/{}", self.prefix.clone(), gen_dir)
        };

        self.list_objects_v2(&gen_prefix)
    }

    async fn read_to_string<E: 'static + std::error::Error>(
        output: Result<GetObjectOutput, E>,
    ) -> Result<String, E> {
        use futures::TryStreamExt;

        let bytes = output?
            .body
            .try_next()
            .await
            .unwrap()   //TODO
            .unwrap();  //TODO

        let string = std::str::from_utf8(&bytes).unwrap();
        Ok(string.into())
    }

    fn get_object_future(&self, key: &str) -> impl Future<Output = ABCDResult<GetObjectOutput>> {
        self.client
            .get_object()
            .bucket(&self.bucket)
            .key(key)
            .send()
            .map_err(Into::<ABCDError>::into)
    }

    fn put_object_future(&self, key: &str, body: &str) -> impl Future<Output = ABCDResult<PutObjectOutput>> {        
        let bytes = ByteStream::from(Bytes::from(body.to_string()));
        
        self.client
            .put_object()
            .bucket(&self.bucket)
            .key(key)
            .body(bytes)
            .acl(ObjectCannedAcl::BucketOwnerFullControl)
            .send()
            .map_err(Into::<ABCDError>::into)
    }
}
impl Storage for S3System {
    fn previous_gen_number(&self) -> ABCDResult<u16> {
        let objects = self.list_objects_v2(&self.prefix)?;

        //TODO compile regex only once for entire struct.
        let re = {
            let string = format!(r#"^{}/gen_(?P<gid1>\d*)/gen_(?P<gid2>\d*).json"#, &self.prefix);
            Regex::new(&string)?
        };
        let key_strings = objects.into_iter().filter_map(|obj| obj.key);
        
        key_strings
            .filter_map(|key| {
                re.captures(&key)
                    .map(|caps| caps["gid1"].parse::<u16>().ok())
                    .flatten()
            })
            .max()
            .ok_or_else(||ABCDError::NoGenZeroDirExists("".into())) //TODO better error
    }

    fn load_previous_gen<P>(&self) -> ABCDResult<Generation<P>>
    where
        P: DeserializeOwned + Debug,
    {
        let object_key = {
            let prev_gen_no = self.previous_gen_number()?;
            let prev_gen_file_dir = format!("gen_{:03}", prev_gen_no);
            let prev_gen_file_name = format!("gen_{:03}.json", prev_gen_no);

            format!(
                "{}/{}/{}",
                self.prefix.clone(),
                prev_gen_file_dir,
                prev_gen_file_name
            )
        };

        let string_fut = self.get_object_future(&object_key).then(Self::read_to_string);
        let string = self.runtime.block_on(string_fut)?;
        let parsed: Generation<P> = serde_json::from_str(&string)?;
        Ok(parsed)
    }

    fn save_particle<P: Serialize>(&self, w: &Particle<P>) -> ABCDResult<String> {
        let object_path = {
            let gen_no = self.previous_gen_number()? + 1;
            let gen_file_dir = format!("gen_{:03}", gen_no);
            let file_uuid = Uuid::new_v4();
            let particle_file_name = file_uuid.to_string() + ".json";
            let prefix_cloned = self.prefix.clone();

            format!("{}/{}/{}", prefix_cloned, gen_file_dir, particle_file_name)
        };

        let pretty_json = serde_json::to_string_pretty(w)?;

        let request = self.put_object_future(
            &object_path, 
            &pretty_json
        );

        self.runtime.block_on(request)?;

        Ok(object_path)
    }

    fn num_working_particles(&self) -> ABCDResult<u32> {
        let files_in_folder = self.get_particle_files_in_active_gen();
        match files_in_folder {
            // Err(_) if self.check_active_gen().ok() == Some(1) => Ok(0),
            Ok(files) => Ok(files.len().try_into()?), //TODO read dir numbers & take max
            Err(e) => Err(e),
        }
    }

    fn load_working_particles<P>(&self) -> ABCDResult<Vec<Particle<P>>>
    where
        P: DeserializeOwned,
    {
        let object_names = self
            .get_particle_files_in_active_gen()?
            .into_iter()
            .map(|t| t.key)
            .collect::<Option<Vec<String>>>()
            .ok_or_else(|| ABCDError::Other("failed to identify all particle file names".into()))?;

        let particle_futures = object_names.into_iter().map(|filename| {
            self.get_object_future(&filename)
                .then(Self::read_to_string)
                .map(|res| {    //TODO better
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
    }

    fn save_new_gen<P: Serialize>(
        &self,
        gen: &Generation<P>
    ) -> ABCDResult<()> {
        let gen_dir = format!("gen_{:03}", gen.number);
        let object_name = format!("gen_{:03}.json", gen.number);
        let prefix_cloned = self.prefix.clone();
        let object_path = format!("{}/{}/{}", prefix_cloned, gen_dir, object_name);
        
        //Test if the file is somehow already there
        let request = self.client
            .get_object_acl()
            .bucket(&self.bucket)
            .key(&object_path)
            .send();

        match self.runtime.block_on(request) {
            Err(SdkError::ServiceError{
                err: GetObjectAclError{..},
                raw: _,
            }) => {
                //This is good, means we're not writing over existing gen
                let request = self.put_object_future(
                    &object_path,
                    &serde_json::to_string_pretty(gen)?
                );
                self.runtime.block_on(request)?;
                Ok(())
            },
            _ => {
                //This is bad, the file shouldn't exist before we've saved it!
                Err(ABCDError::GenAlreadySaved(format!(
                    "Gen file already existed at {:?}",
                    object_path
                )))
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{path::Path, fs::DirEntry};

    use aws_sdk_s3::{Region, model::{Delete, ObjectIdentifier}};
    use futures::TryStreamExt;
    use serde_json::Value;

    use crate::{etc::config::Config, storage::test_helper::{make_dummy_generation, DummyParams}, test_helper::test_data_path, Population};

    use super::*;

    struct StorageTestHelper{
        bucket: String,
        prefix: String,
        delete_prefix_on_drop: bool,
        client: Client,
        runtime: Runtime,
    }
    impl StorageTestHelper {
        pub fn new(storage: &S3System, delete_prefix_on_drop: bool) -> Self {
            let runtime = Runtime::new().unwrap();

            let config = runtime.block_on(
                aws_config::from_env().region(Region::new("eu-west-1")).load()
            );
            let client = Client::new(&config);

            let instance = StorageTestHelper{
                bucket: storage.bucket.clone(),
                prefix: storage.prefix.clone(),
                delete_prefix_on_drop,
                client,
                runtime,
            };

            //Delete anything that happens to already be in there
            instance.delete_prefix_recursively();

            instance
        }

        fn put_recursive(&self, local_path: &Path) {
            fn visit_dirs(dir: &Path, cb: &dyn Fn(&DirEntry)) -> std::io::Result<()> {
                if dir.is_dir() {
                    for entry in std::fs::read_dir(dir)? {
                        let entry = entry?;
                        let path = entry.path();
                        if path.is_dir() {
                            visit_dirs(&path, cb)?;
                        } else {
                            cb(&entry);
                        }
                    }
                }
                Ok(())
            }

            let prefix = Path::new(&self.prefix);
           
            let uploader = |de: &DirEntry|{
                let absolute_path = de.path();
                let stripped_path = absolute_path.strip_prefix(local_path).unwrap();
                let object_name = prefix.join(&stripped_path).to_string_lossy().into_owned();
                let file_contents = std::fs::read_to_string(&absolute_path).unwrap();
                self.put_object(
                    &object_name,
                    &file_contents
                );
            };
            visit_dirs(local_path, &uploader).unwrap();
        }

        fn put_object(&self, key: &str, body: &str) {
            let bytes = ByteStream::from(Bytes::from(body.to_string()));

            let t = self.runtime.block_on(async {
                self.client
                    .put_object()
                    .bucket(&self.bucket)
                    .acl(ObjectCannedAcl::BucketOwnerFullControl)
                    .key(key)
                    .body(bytes)
                    .send()
                    .await
                    .unwrap()
            });
        }

        fn get_object(&self, key: &str) -> String {
            self.runtime.block_on(async{
                let bytes = self.client
                    .get_object()
                    .bucket(&self.bucket)
                    .key(&format!("{}/{}", self.prefix, key))
                    .send()
                    .await
                    .unwrap()
                    .body
                    .try_next()
                    .await
                    .unwrap()
                    .unwrap();
    
                std::str::from_utf8(&bytes).unwrap().into()
            })
        }

        // fn get_particle(&self, key: &str) -> Particle<DummyParams> {
        //     let string = self.get_object(key);
        //     serde_json::from_str(&string).unwrap()
        // }

        fn delete_prefix_recursively(&self) {
            if self.delete_prefix_on_drop {
                self.runtime.block_on(async {
                    let object_identifiers: Vec<_> = self.client
                        .list_objects_v2()
                        .bucket(&self.bucket)
                        .prefix(&self.prefix)
                        .send()
                        .map(|response| {
                            response.unwrap().contents.unwrap_or_default()
                        })
                        .await
                        .into_iter()
                        .map(|o|{
                            ObjectIdentifier::builder().set_key(o.key).build()
                        })
                        .collect();
    
                    if object_identifiers.is_empty() { return }

                    self.client
                        .delete_objects()
                        .bucket(&self.bucket)
                        .delete(Delete::builder().set_objects(Some(object_identifiers)).build())
                        .send()
                        .await
                        .unwrap();

                    let remaining = self.client
                        .list_objects_v2()
                        .bucket(&self.bucket)
                        .prefix(&self.prefix)
                        .send()
                        .await
                        .unwrap();

                    match remaining.key_count {
                        0 => (),
                        _ => panic!("Failed to delete all objects")
                    };
                })
            }
        }
    }
    impl Drop for StorageTestHelper {
        fn drop(&mut self) {
            if self.delete_prefix_on_drop {
                self.delete_prefix_recursively();
            }
        }
    }

    fn storage() -> S3System {
        let path = test_data_path("resources/test/config_test.toml");
        Config::from_path(path).storage.build_s3()
    }

    #[test]
    fn test_previous_gen_num_three() {
        let storage = storage();

        let helper = StorageTestHelper::new(&storage, true);
        helper.put_recursive(&test_data_path("resources/test/fs/example"));

        assert_eq!(2, storage.previous_gen_number().unwrap());
    }

    #[test]
    fn test_previous_gen_num_zero() {
        let storage = storage();

        let helper = StorageTestHelper::new(&storage, true);
        helper.put_recursive(&test_data_path("resources/test/fs/example_gen0"));

        assert_eq!(0, storage.previous_gen_number().unwrap());
    }

    #[test]
    fn test_retrieve_previous_gen() {
        let gen_number = 3;
        let expected = make_dummy_generation(gen_number, 0.3);
        let storage = storage();

        let helper = StorageTestHelper::new(&storage, true);
        helper.put_object(
            &format!("{}/gen_003/gen_003.json", storage.prefix),
            &serde_json::to_string_pretty(&expected).unwrap()
        );

        let result = storage
            .load_previous_gen::<DummyParams>()
            .unwrap();

        assert_eq!(expected, result);
    }

    #[test]
    fn test_exception_if_gen_num_doesnt_match_path() {
        let gen_number = 999;
        let expected = make_dummy_generation(gen_number, 0.3);
        let storage = storage();

        let helper = StorageTestHelper::new(&storage, true);
        helper.put_object(
            &format!("{}/gen_003/gen_003.json", storage.prefix),
            &serde_json::to_string_pretty(&expected).unwrap()
        );

        let result = storage.load_previous_gen::<DummyParams>();

        //TODO proper error, e.g. "InconsistentFileSystem(msg)"
        assert!(result.is_err());
    }

    #[test]
    fn test_exception_if_save_but_no_gen_zero_exists() {
        todo!();
    }

    #[test]
    fn test_save_particle() {
        let storage = storage();
        let helper = StorageTestHelper::new(&storage, true);

        let w1 = Particle {
            parameters:  DummyParams::new(1, 2.),
            scores: vec![100.0, 200.0],
            weight: 1.234,
        };

        // TODO fix the async problem like this:
        // https://github.com/hyperium/hyper/issues/2112
        // or rusoto you can create a new client https://docs.rs/rusoto_core/0.43.0/rusoto_core/request/struct.HttpClient.html
        // from here and then pass that into the specific service's constructor. This will avoid using the lazy_static client.
        let saved_1 = storage.save_particle(&w1).unwrap();
        let loaded: Particle<DummyParams> = serde_json::from_str(&helper.get_object(&saved_1)).unwrap();

        assert_eq!(w1, loaded);
    }

    #[test]
    fn test_exception_saving_inconsistent_gen_number(){
        todo!()
    }

    #[test]
    fn test_number_particle_files() {
        let storage = storage();

        let helper = StorageTestHelper::new(&storage, true);
        helper.put_recursive(&test_data_path("resources/test/fs/example"));

        assert_eq!(2, storage.num_working_particles().unwrap())
    }

    #[test]
    fn test_retrieve_particle_files() {
        let storage = storage();

        let helper = StorageTestHelper::new(&storage, true);
        helper.put_recursive(&test_data_path("resources/test/fs/example"));

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

        let mut result: Vec<Particle<DummyParams>> = storage.load_working_particles().unwrap();

        //Sort by weight for easy comparison
        expected.sort_by(|a, b| a.weight.partial_cmp(&b.weight).unwrap());
        result.sort_by(|a, b| a.weight.partial_cmp(&b.weight).unwrap());

        assert_eq!(expected, result);
    }

    #[test]
    fn test_save_generation() {
        let storage = storage();

        // Put some test files there, so active gen should be 3.
        let helper = StorageTestHelper::new(&storage, true);
        helper.put_recursive(&test_data_path("resources/test/fs/example"));
        
        let gen_number = 3;
        let dummy_generation = make_dummy_generation(gen_number, 0.3);

        storage
            .save_new_gen(&dummy_generation)
            .expect("Expected successful save");

        let expected = dummy_generation;

        //Manually load what was saved to have a look
        let actual: Generation<DummyParams> = serde_json::from_str(
            &helper.get_object(&format!("gen_{:03}/gen_{:03}.json", 3, 3))
        ).unwrap();

        assert_eq!(expected, actual);
    }

    #[test]
    fn test_load_generation() {
        let storage = storage();

        // Put some test files there, so active gen should be 3.
        let helper = StorageTestHelper::new(&storage, true);
        helper.put_recursive(&test_data_path("resources/test/fs/example"));
        
        let expected = Generation{
            pop: Population{ 
                tolerance: 0.1234, 
                acceptance: 0.7, 
                normalised_particles: vec![
                    Particle{ 
                        parameters: DummyParams::new(10, 20.0), 
                        scores: vec![1000.0, 2000.0], 
                        weight: 0.234 
                    },
                    Particle{ 
                        parameters: DummyParams::new(30, 40.0), 
                        scores: vec![3000.0, 4000.0], 
                        weight: 0.567 
                    }
                ] 
            },
            number: 2,
        };

        let actual = storage.load_previous_gen().unwrap();

        assert_eq!(expected, actual);
    }

    // #[test]
    // fn dont_save_over_existing_gen_file(){
    //     let gen_number = 4;
        
    //     let dummy_gen_1 = make_dummy_generation(gen_number, 0.3);
    //     let dummy_gen_2 = make_dummy_generation(gen_number, 0.4);
        
    //     let storage = storage();

    //     let helper = StorageTestHelper::new(&storage, true);
    //     helper.put_recursive(&test_data_path("resources/test/fs/example"));

    //     //1. Save an dummy gen_003 file, representing file already save by another node
    //     storage.save_new_gen(&dummy_gen_1).expect("Expected successful save");

    //     //2. Try to save another gen over it, pretending we didn't notice the other node save gen before us
    //     let outcome = storage.save_new_gen(&dummy_gen_2);
    //     match outcome {
    //         Err(ABCDError::GenAlreadySaved(_)) => (),
    //         other => panic!("Expected error, got: {:?}", other)
    //     };

    //     //3. Test that the original file save by other node is intact and we didn't panic.
    //     let loaded = {
    //         let string = test_storage.get_object("gen_003/gen_003.json");
    //         serde_json::from_str(&string).unwrap()
    //     };
    //     assert_eq!(dummy_gen_1, loaded);
    // }
}
