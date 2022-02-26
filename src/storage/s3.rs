use aws_sdk_s3::error::GetObjectAclError;
use aws_sdk_s3::{Client, SdkError, ByteStream, Region};
use aws_sdk_s3::model::{Object, ObjectCannedAcl, BucketVersioningStatus, ObjectIdentifier, Delete};
use aws_sdk_s3::output::{GetObjectOutput, PutObjectOutput, ListObjectsV2Output};
use bytes::Bytes;
use futures::{FutureExt, Future, TryFutureExt, TryStreamExt};
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

        let instance = S3System {
            bucket,
            prefix,
            client,
            runtime,
            gen_zero_re,
            gen_non_zero_re,
        };

        instance.runtime.block_on(instance.assert_versioning_active())?;

        Ok(instance)
    }

    async fn assert_versioning_active(&self) -> ABCDResult<()> { 
        let enabled = self.client.get_bucket_versioning()
            .bucket(&self.bucket)
            .send()
            .await?
            .status
            .map(|s|s == BucketVersioningStatus::Enabled)
            .unwrap_or(false);

        if enabled { Ok(()) }
        else { Err(ABCDError::S3OperationError("Versioning must be enabled".into())) }
    }

    async fn list_objects_v2(&self, prefix: &str) -> ABCDResult<Vec<Object>> {
        let mut acc: Vec<Object> = Vec::new();

        async fn next_page(client: &Client, bucket: &str, prefix: &str, c_tok: Option<String>) -> ABCDResult<ListObjectsV2Output> {
            client
                .list_objects_v2()
                .bucket(bucket)
                .prefix(prefix)
                .set_continuation_token(c_tok)
                .send()
                .await
                .map_err(|e|e.into())
        }

        let mut c_token = None;
        loop {
            let list_output = next_page(&self.client, &self.bucket, prefix, c_token).await?;
            if let Some(mut items) = list_output.contents {
                acc.append(&mut items);
            }
            
            c_token = list_output.continuation_token;
            if c_token.is_none() { break; }
        }

        Ok(acc)
    }

    async fn get_particle_files_in_active_gen(&self) -> ABCDResult<Vec<Object>> {
        let gen_prefix = {
            let gen_no = self.previous_gen_number_async().await? + 1;
            let gen_dir = format!("gen_{:03}", gen_no);
            format!("{}/{}", self.prefix.clone(), gen_dir)
        };

        self.list_objects_v2(&gen_prefix).await
    }

    async fn read_to_string(
        output: ABCDResult<GetObjectOutput>,
    ) -> ABCDResult<String> {
        // use futures::TryStreamExt;

        let bytes = output?
            .body
            .try_next()
            .await?
            .ok_or_else(||
                ABCDError::S3OperationError("Empty byte stream".into())
            )?;

        let string = std::str::from_utf8(&bytes).unwrap();
        Ok(string.into())
    }

    async fn ensure_only_original_verions(&self, key: &str) -> ABCDResult<String> {
        let list_obj_ver = self.client
            .list_object_versions()
            .bucket(&self.bucket)
            .prefix(key)
            .send()
            .await?;

        if list_obj_ver.is_truncated {
            return Err(ABCDError::S3OperationError(
                format!("Too many object verions - pagination not currently in use: {:?}", list_obj_ver)
            ));
        }

        let mut versions = list_obj_ver.versions.unwrap_or_default();
        let delete_markers =  list_obj_ver.delete_markers.unwrap_or_default();

        if versions.len() == 1 && delete_markers.is_empty() {
            if let Some(version) = versions.swap_remove(0).version_id {
                return Ok(version);
            } else {
                return Err(ABCDError::S3OperationError(format!("Only verion of {} has ID None", key)));
            }
        }

        let oldest_version_id = 
            if let Some(version) = versions.pop().and_then(|ov|ov.version_id) {
                version
            } else {
                return Err(ABCDError::S3OperationError(format!("Oldest verion of {} has ID None", key)));
            };

        let vers_to_delete = versions.into_iter()
            .filter_map(|ov|
                if ov.key.is_some() && ov.version_id.is_some() {
                    Some((ov.key.unwrap(), ov.version_id.unwrap()))
                } else {
                    None
                }
            );
        let dms_to_delete = delete_markers.into_iter()
            .filter_map(|ov|
                if ov.key.is_some() && ov.version_id.is_some() {
                    Some((ov.key.unwrap(), ov.version_id.unwrap()))
                } else {
                    None
                }
            );
        let to_delete: Vec<ObjectIdentifier> = {
            vers_to_delete.chain(dms_to_delete)
                .map(|(key, id)| 
                    ObjectIdentifier::builder()
                        .set_version_id(Some(id))
                        .set_key(Some(key))
                        .build()
                )
                .collect()
        };

        self.client
            .delete_objects()
            .bucket(&self.bucket)
            .delete(Delete::builder().set_objects(Some(to_delete)).build())
            .send()
            .await?;

        Ok(oldest_version_id)
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

    async fn previous_gen_number_async(&self) -> ABCDResult<u16> {
        let objects = self.list_objects_v2(&self.prefix).await?;

            if !objects.iter()
                    .filter_map(|o|{
                        o.key.as_ref()
                    })
                    .any(|k|self.gen_zero_re.is_match(k)) {
                return Err(ABCDError::StorageInitError)
            }
    
            let key_strings = objects.into_iter().filter_map(|obj| obj.key);
            
            let gen_number = key_strings
                .filter_map(|key| {
                    self.gen_non_zero_re.captures(&key)
                        .map(|caps| caps["gid1"].parse::<u16>().ok())
                        .flatten()
                })
                .max()
                .unwrap_or(0);
    
            Ok(gen_number)
    }
}
impl Storage for S3System {
    fn previous_gen_number(&self) -> ABCDResult<u16> {
        self.runtime.block_on(self.previous_gen_number_async())
    }

    fn load_previous_gen<P>(&self) -> ABCDResult<Generation<P>>
    where
        P: DeserializeOwned + Debug,
    {
        self.runtime.block_on(async{
            let prev_gen_no = self.previous_gen_number_async().await?;
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
    
            let version_id = self.ensure_only_original_verions(&object_key).await?;
    
            let obj_string = self.client
                .get_object()
                .bucket(&self.bucket)
                .key(&object_key)
                .version_id(version_id)
                .send()
                .map_err(Into::<ABCDError>::into)
                .then(Self::read_to_string)
                .await?;
            
            let gen: Generation<P> = serde_json::from_str(&obj_string)?;
    
            if gen.number == prev_gen_no { Ok(gen) } 
            else {
                Err(ABCDError::StorageConsistencyError(
                    format!("Expected gen number {} but got {}", prev_gen_no, gen.number)
                ))
            }
        })
    }

    fn save_particle<P: Serialize>(&self, w: &Particle<P>) -> ABCDResult<String> {
        self.runtime.block_on(async {
            let gen_file_dir = {
                let gen_no = self.previous_gen_number_async().await? + 1;
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
    
            self.put_object_future(
                &prefixed_path, 
                &pretty_json
            ).await?;
    
            Ok(non_prefixed_path)
        })
    }

    fn num_working_particles(&self) -> ABCDResult<u32> {
        let files_in_folder = self.runtime
            .block_on(self.get_particle_files_in_active_gen());
        
        match files_in_folder {
            // Err(_) if self.check_active_gen().ok() == Some(1) => Ok(0),
            Ok(files) => Ok(files.len().try_into()?), //TODO read dir numbers & take max
            Err(e) => Err(e),
        }
    }

    fn load_working_particles<P>(&self) -> ABCDResult<Vec<Particle<P>>>
    where P: DeserializeOwned,
    {
        self.runtime.block_on(async {
            let object_names = self
                .get_particle_files_in_active_gen().await?
                .into_iter()
                .map(|t| t.key)
                .collect::<Option<Vec<String>>>()
                .ok_or_else(|| ABCDError::Other("failed to identify all particle file names".into()))?;

            let particle_futures = object_names.into_iter().map(|filename| {
                self.client
                    .get_object()
                    .bucket(&self.bucket)
                    .key(&filename)
                    .send()
                    .map_err(Into::<ABCDError>::into)
                    .then(Self::read_to_string)
                    .map(|res| {
                        res.and_then(move |s| {
                            serde_json::from_str::<Particle<P>>(&s)
                                .map_err(|e| ABCDError::SerdeError(
                                    format!("Failed to deserialise {}: {}", filename.clone(), e)
                                ))
                        })
                    }) 
            });

            let joined = futures::future::join_all(particle_futures);
            let particles: Vec<Result<Particle<P>, ABCDError>> = joined.await;

            let result_of_vec: ABCDResult<Vec<Particle<P>>> = particles.into_iter().collect();
            result_of_vec
        }) 
    }

    fn save_new_gen<P: Serialize>(
        &self,
        gen: &Generation<P>
    ) -> ABCDResult<()> {
        self.runtime.block_on(async{
            let expected_new_gen_number = self.previous_gen_number_async().await? + 1;
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
            let get_acl_output = self.client
                .get_object_acl()
                .bucket(&self.bucket)
                .key(&object_path)
                .send()
                .await;
    
            match get_acl_output {
                Err(SdkError::ServiceError{
                    err: GetObjectAclError{..},
                    raw: _,
                }) => {
                    //This is good, means we're not writing over existing gen
                    self.put_object_future(
                        &object_path,
                        &serde_json::to_string_pretty(gen)?
                    ).await?;
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
        })
    }
}

#[cfg(test)]
mod tests {
    use std::{path::Path, fs::DirEntry};

    use aws_sdk_s3::{Region, model::{Delete, ObjectIdentifier}};
    use futures::TryStreamExt;

    use crate::{storage::{test_helper::{DummyParams, make_dummy_generation}, config::StorageConfig}, test_helper::test_data_path, Population};

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

        fn list_objects_under(&self, sub_prefix: Option<&str>) -> Vec<Object> {
            let prefix = sub_prefix.map(|p|format!("{}/{}", self.prefix, p))
                .unwrap_or_else(||self.prefix.clone());
            
            let response = self.runtime
                .block_on({
                    self.client
                        .list_objects_v2()
                        .bucket(&self.bucket)
                        .prefix(&prefix)
                        .send()
                });

            let response = response.expect("Expected list objects response");
            assert!(response.continuation_token.is_none());
            response.contents.unwrap_or_default()
        }

        fn delete_prefix_recursively(&self) {
            if self.delete_prefix_on_drop {
                if self.list_objects_under(None)
                        .into_iter()
                        .map(|o|{
                            ObjectIdentifier::builder().set_key(o.key).build()
                        })
                        .next()
                        .is_none() { 
                    return 
                }
                
                self.runtime.block_on(async {
                    let list_obj_ver = self.client
                        .list_object_versions()
                        .bucket(&self.bucket)
                        .prefix(&self.prefix)
                        .send()
                        .await
                        .unwrap();

                    let ver_markers = list_obj_ver.versions
                        .unwrap_or_default()
                        .into_iter()
                        .filter_map(|v|{
                            if v.key.is_some() && v.version_id.is_some() {
                                Some((v.key.unwrap(), v.version_id.unwrap()))
                            } else {
                                None
                            }
                        });
                    let del_markers = list_obj_ver.delete_markers
                        .unwrap_or_default()
                        .into_iter()
                        .filter_map(|v|{
                            if v.key.is_some() && v.version_id.is_some() {
                                Some((v.key.unwrap(), v.version_id.unwrap()))
                            } else {
                                None
                            }
                        });

                    let object_identifiers: Vec<ObjectIdentifier> = ver_markers.chain(del_markers)
                        .map(|(key, id)| 
                            ObjectIdentifier::builder()
                                .set_version_id(Some(id))
                                .set_key(Some(key))
                                .build()
                        )
                        .collect();

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
                        _ => panic!("Failed to delete all objects: {:?}", &remaining)
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

    fn gen_002() -> Generation<DummyParams> {
        Generation{
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
        }
    }

    #[test]
    fn test_previous_gen_num_three() {
        let instance = storage_using_prefix("test_previous_gen_num_three");

        let helper = StorageTestHelper::new(&instance, true);
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
    fn test_multiple_gen() {
        let instance = storage_using_prefix("test_multiple_gen");

        let helper = StorageTestHelper::new(&instance, true);
        helper.put_recursive("resources/test/storage/example");

        // Manually upload another gen_002 file, as though
        // it was uploaded concurrently by another node.
        helper.put_object(
            &format!("{}/{}", &helper.prefix, "gen_002/gen_002.json"), 
            "Contents of an overwritten gen file"
        );

        let loaded: Generation<DummyParams> = instance.load_previous_gen().unwrap();

        assert_eq!(gen_002(), loaded);
    }

    #[test]
    fn test_load_previous_gen() {
        let instance = storage_using_prefix("test_load_previous_gen");

        let helper = StorageTestHelper::new(&instance, true);
        helper.put_recursive("resources/test/storage/example");

        let result = instance
            .load_previous_gen::<DummyParams>()
            .unwrap();

        

        assert_eq!(gen_002(), result);
    }

    // #[test]
    // fn test_version_experiments() {
    //     let instance = storage_using_prefix("test_versioning");

    //     let helper = StorageTestHelper::new(&instance, true);
    //     helper.put_recursive("resources/test/storage/example");
    //     helper.put_recursive("resources/test/storage/example");

    //     instance.get_ensuring_version_one("test_versioning/abcd.init");
    // }

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

        let loaded: Particle<DummyParams> = serde_json::from_str(
            &helper.get_object(&save_path)
        ).unwrap();

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

        let helper = StorageTestHelper::new(&instance, false);
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
