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
    gen_zero_re: Regex,
    gen_non_zero_re: Regex
}
impl S3System {
    pub fn new(bucket: String, prefix: String) -> ABCDResult<Self> {
        let runtime = Runtime::new().unwrap();
        let client = {
            let config = runtime.block_on(
                aws_config::from_env().region(Region::new("eu-west-1")).load()
            );
            Client::new(&config)
        };

        let gen_zero_re = {
            let string = format!(r#"^{}/abcd.init"#, &prefix);
            Regex::new(&string)?
        };

        let gen_non_zero_re = {
            let string = format!(r#"^{}/gen_(?P<gid1>\d*)/gen_(?P<gid2>\d*).json"#, &prefix);
            Regex::new(&string)?
        };

        Ok(S3System {
            bucket,
            prefix,
            client,
            runtime,
            gen_zero_re,
            gen_non_zero_re,
        })
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
            let gen_no = self.previous_gen_number()? + 1;
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

        if !objects.iter()
                .filter_map(|o|{
                    o.key.as_ref()
                })
                .any(|k|self.gen_zero_re.is_match(k)) {
            return Err(ABCDError::StorageInitError)
        }

        let key_strings = objects.into_iter().filter_map(|obj| obj.key);
        
        //TODO there is nothing currently checking that gid1 == gid2, which might lead to a consistnecy error
        let gen_number = key_strings
            .filter_map(|key| {
                self.gen_non_zero_re.captures(&key)
                    .map(|caps| caps["gid1"].parse::<u16>().ok())
                    .flatten()
            })
            .max()
            .unwrap_or(0);

            // let file_number = key_strings
            // .filter_map(|key| {
            //     self.gen_non_zero_re.captures(&key)
            //         .map(|caps| caps["gid2"].parse::<u16>().ok())
            //         .flatten()
            // })
            // .max()
            // .unwrap_or(0);

            // if(gen_number!=file_number){
            //     return Err(ABCDError::StorageInitError) //To Test this do we need to set up new directory structure where gen dir number and gen file number are different
            // }

        Ok(gen_number)
    }

    fn load_previous_gen<P>(&self) -> ABCDResult<Generation<P>>
    where
        P: DeserializeOwned + Debug,
    {
        let prev_gen_no = self.previous_gen_number()?;
        let object_key = {
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
        let gen: Generation<P> = serde_json::from_str(&string)?;

        if gen.number == prev_gen_no {
            Ok(gen)
        } else {
            Err(ABCDError::StorageConsistencyError(
                format!("Expected gen number {} but got {}", prev_gen_no, gen.number)
            ))
        }
    }

    fn save_particle<P: Serialize>(&self, w: &Particle<P>) -> ABCDResult<String> {
        let gen_file_dir = {
            let gen_no = self.previous_gen_number()? + 1;
            format!("gen_{:03}", gen_no)
        };
        
        let particle_file_name = {
            let file_uuid = Uuid::new_v4();
            file_uuid.to_string() + ".json"
        };

        let non_prefixed_path =format!(
            "{}/{}",  
            gen_file_dir, 
            particle_file_name
        );

        let prefixed_path = format!(
            "{}/{}", 
            self.prefix.clone(), 
            non_prefixed_path
        );

        let pretty_json = serde_json::to_string_pretty(w)?;

        let request = self.put_object_future(
            &prefixed_path, 
            &pretty_json
        );

        self.runtime.block_on(request)?;

        Ok(non_prefixed_path)
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
        let expected_new_gen_number = self.previous_gen_number()? + 1;
        if gen.number != expected_new_gen_number {
            return Err(
                ABCDError::StorageConsistencyError(
                    format!(
                        "Asked to save gen {}, but was due to save {}", 
                        &gen.number, 
                        &expected_new_gen_number
                    )
                )
            )
        }

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

    use crate::{storage::{test_helper::{DummyParams, make_dummy_generation}, config::StorageConfig}, test_helper::test_data_path, types::Population};

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

        fn put_recursive(&self, proj_path: &str) {
            let abs_project_path = &test_data_path(proj_path);

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
                let stripped_path = absolute_path.strip_prefix(abs_project_path).unwrap();
                let object_name = prefix.join(&stripped_path).to_string_lossy().into_owned();
                let file_contents = std::fs::read_to_string(&absolute_path).unwrap();
                self.put_object(
                    &object_name,
                    &file_contents
                );
            };
            visit_dirs(abs_project_path, &uploader).unwrap();
        }

        fn put_object(&self, key: &str, body: &str) {
            let bytes = ByteStream::from(Bytes::from(body.to_string()));

            self.runtime.block_on(async {
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

    fn storage_using_prefix(prefix: &str) -> S3System {
        if !envmnt::exists("TEST_BUCKET") {
            panic!("You need to set the environment variable 'TEST_BUCKET' before running");
        }

        let storage_cfg = StorageConfig::S3{
            bucket: "${TEST_BUCKET}".into(),
            prefix: prefix.into(),
        };

        storage_cfg
            .build_s3()
            .expect("Failed to bulid storage instance")
    }

    #[test]
    fn test_previous_gen_num_three() {
        let instance = storage_using_prefix("test_previous_gen_num_three");

        let helper = StorageTestHelper::new(&instance, false);
        helper.put_recursive("resources/test/storage/example");

        assert_eq!(2, instance.previous_gen_number().unwrap());
    }

    #[test]
    fn test_previous_gen_num_zero() {
        let instance = storage_using_prefix("test_previous_gen_num_zero");

        let helper = StorageTestHelper::new(&instance, true);
        helper.put_recursive("resources/test/storage/example_gen0");

        assert_eq!(0, instance.previous_gen_number().unwrap());
    }

    #[test]
    fn test_load_previous_gen() {
        let instance = storage_using_prefix("test_load_previous_gen");

        let helper = StorageTestHelper::new(&instance, true);
        helper.put_recursive("resources/test/storage/example");

        let result = instance
            .load_previous_gen::<DummyParams>()
            .unwrap();

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

        assert_eq!(expected, result);
    }

    #[test]
    fn test_exception_if_load_gen_not_matching_path() {
        let instance = storage_using_prefix("test_exception_if_load_gen_not_matching_path");

        let helper = StorageTestHelper::new(&instance, true);
        helper.put_recursive("resources/test/storage/example_gen_mismatch");

        let result = instance.load_previous_gen::<DummyParams>();

        match result {
            Err(ABCDError::StorageConsistencyError(_)) => (),
            _ => panic!("Expected exception")
        }
    }

    #[test]
    fn test_exception_if_save_without_init() {
        let instance = storage_using_prefix("test_exception_if_save_without_init");

        let helper = StorageTestHelper::new(&instance, true);
        helper.put_recursive("resources/test/storage/empty");

        let w1 = Particle {
            parameters:  DummyParams::new(1, 2.),
            scores: vec![100.0, 200.0],
            weight: 1.234,
        };

        let result = instance.save_particle(&w1);

        match result {
            Err(ABCDError::StorageInitError) => (),
            _ => panic!("Expected exception")
        }
    }

    #[test]
    fn test_save_particle() {
        let instance = storage_using_prefix("test_save_particle");

        let helper = StorageTestHelper::new(&instance, true);
        helper.put_recursive("resources/test/storage/example");

        let w1 = Particle {
            parameters:  DummyParams::new(1, 2.),
            scores: vec![100.0, 200.0],
            weight: 1.234,
        };

        let save_path = instance.save_particle(&w1).unwrap();

        let loaded: Particle<DummyParams> = serde_json::from_str(&helper.get_object(&save_path)).unwrap();

        assert_eq!(w1, loaded);
    }

    #[test]
    fn test_exception_if_save_inconsistent_gen_number(){
        let instance = storage_using_prefix("test_exception_saving_inconsistent_gen_number");

        let helper = StorageTestHelper::new(&instance, true);
        helper.put_recursive("resources/test/storage/example");

        let dummy_generation = make_dummy_generation(999, 0.3);

        let result = instance
            .save_new_gen(&dummy_generation);

        match result {
            Err(ABCDError::StorageConsistencyError(_)) => (),
            _ => panic!("Expected exception")
        }
    }

    #[test]
    fn test_num_working_particles() {
        let instance = storage_using_prefix("test_num_working_particles");

        let helper = StorageTestHelper::new(&instance, true);
        helper.put_recursive("resources/test/storage/example");

        assert_eq!(2, instance.num_working_particles().unwrap())
    }

    #[test]
    fn test_load_working_particles() {
        let instance = storage_using_prefix("test_load_working_particles");

        let helper = StorageTestHelper::new(&instance, true);
        helper.put_recursive("resources/test/storage/example");

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
    fn test_save_generation() {
        let instance = storage_using_prefix("test_save_generation");

        let helper = StorageTestHelper::new(&instance, true);
        helper.put_recursive("resources/test/storage/example");
        
        let gen_number = 3;
        let dummy_generation = make_dummy_generation(gen_number, 0.3);

        instance
            .save_new_gen(&dummy_generation)
            .expect("Expected successful save");

        let expected = dummy_generation;

        //Manually load what was saved to S3 to check
        let actual: Generation<DummyParams> = serde_json::from_str(
            &helper.get_object(&format!("gen_{:03}/gen_{:03}.json", 3, 3))
        ).unwrap();

        assert_eq!(expected, actual);
    }

    #[test]
    fn test_load_generation() {
        let instance = storage_using_prefix("test_load_generation");

        let helper = StorageTestHelper::new(&instance, true);
        helper.put_recursive("resources/test/storage/example");
        
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

        let actual = instance.load_previous_gen().unwrap();

        assert_eq!(expected, actual);
    }
}
