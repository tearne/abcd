#[cfg(test)]
mod tests;

use aws_sdk_s3::error::GetObjectAclError;
use aws_sdk_s3::model::{
    BucketVersioningStatus, Delete, Object, ObjectCannedAcl, ObjectIdentifier, DeleteMarkerEntry, ObjectVersion
};
use aws_sdk_s3::output::{GetObjectOutput, ListObjectsV2Output, PutObjectOutput, ListObjectVersionsOutput};
use aws_sdk_s3::types::{ByteStream, SdkError};
use aws_sdk_s3::{Client, Region};
use bytes::Bytes;
use futures::{Future, FutureExt, TryFutureExt};
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
    client: Client,
    pub(super) runtime: Runtime,
    pub prefix: String, 
    particle_prefix: String, 
    completed_prefix: String, 
    completed_gen_re: Regex,
}
impl S3System {
    pub fn new(bucket: String, prefix: String) -> ABCDResult<Self> {
        let runtime = Runtime::new().unwrap();
        let client = {
            let config = runtime.block_on(
                aws_config::from_env()
                    .region(Region::new("eu-west-1"))
                    .load(),
            );
            Client::new(&config)
        };

        let completed_prefix = format!(
            "{}/completed",
            &prefix,
        );

        let particle_prefix = format!(
            "{}/particles",
            &prefix
        );

        let completed_gen_re = {
            let string = format!(r#"^{}/completed/gen_(?P<gid>\d*).json"#, &prefix);
            Regex::new(&string)?
        };

        let instance = S3System {
            bucket,
            client,
            runtime,
            prefix, 
            particle_prefix,
            completed_prefix,
            completed_gen_re,
        };
        instance
            .runtime
            .block_on(instance.assert_versioning_active())?;
        Ok(instance)
    }

    pub fn purge_all_versions_of_everything_in_prefix(&self) -> ABCDResult<()> {
        self.runtime.block_on(async{
            let list_obj_ver = self.get_versions(&self.prefix).await?;

            let ver_markers = list_obj_ver
                    .versions
                    .unwrap_or_default()
                    .into_iter()
                    .filter_map(|v| {
                        if v.key.is_some() && v.version_id.is_some() {
                            Some((v.key.unwrap(), v.version_id.unwrap()))
                        } else {
                            None
                        }
                    });
            let del_markers = list_obj_ver
                .delete_markers
                .unwrap_or_default()
                .into_iter()
                .filter_map(|v| {
                    if v.key.is_some() && v.version_id.is_some() {
                        Some((v.key.unwrap(), v.version_id.unwrap()))
                    } else {
                        None
                    }
                });

            let object_identifiers: Vec<ObjectIdentifier> = ver_markers
                .chain(del_markers)
                .map(|(key, id)| {
                    ObjectIdentifier::builder()
                        .set_version_id(Some(id))
                        .set_key(Some(key))
                        .build()
                })
                .collect();

            self.client
                .delete_objects()
                .bucket(&self.bucket)
                .delete(
                    Delete::builder()
                        .set_objects(Some(object_identifiers))
                        .build(),
                )
                .send()
                .await
                .expect("delete objects failed");

            let remaining = self
                .client
                .list_objects_v2()
                .bucket(&self.bucket)
                .prefix(&self.prefix)
                .send()
                .await
                .unwrap();

            match remaining.key_count {
                0 => Ok(()),
                _ => panic!("Failed to delete all objects: {:?}", &remaining),
            }
        })
    }

    async fn assert_versioning_active(&self) -> ABCDResult<()> {
        let enabled = self
            .client
            .get_bucket_versioning()
            .bucket(&self.bucket) //NOTE: self.bucket gives s3://s3-ranch-007 when all it wants is the name s3-ranch-007
            //.bucket("s3-ranch-007") //NOTE: self.bucket gives s3://s3-ranch-007 when all it wants is the name s3-ranch-007
            .send()
            .await?
            .status
            .map(|s| s == BucketVersioningStatus::Enabled)
            .unwrap_or(false);
        if enabled {
            Ok(())
        } else {
            Err(ABCDError::S3OperationError(
                "Versioning must be enabled".into(),
            ))
        }
    }

    async fn list_objects_v2(&self, prefix: &str) -> ABCDResult<Vec<Object>> {
        let mut acc: Vec<Object> = Vec::new();

        async fn next_page(
            client: &Client,
            bucket: &str,
            prefix: &str,
            c_tok: Option<String>,
        ) -> ABCDResult<ListObjectsV2Output> {
            client
                .list_objects_v2()
                .bucket(bucket)
                .prefix(prefix)
                .set_continuation_token(c_tok)
                .send()
                .await
                .map_err(|e| e.into())
        }

        let mut c_token = None;
        loop {
            let list_output = next_page(&self.client, &self.bucket, prefix, c_token).await?;
            if let Some(mut items) = list_output.contents {
                acc.append(&mut items);
            }

            c_token = list_output.continuation_token;
            if c_token.is_none() {
                break;
            }
        }

        Ok(acc)
    }

    fn assert_only_json(objects: &[Object], prefix: &str) -> ABCDResult<()> {
        let is_not_json_file = |o:&Object|{
            o.key
                .as_ref()
                .map(|k|!k.ends_with(".json"))
                .unwrap_or(true)
        };

        if objects.iter().any(is_not_json_file) {
            Err(ABCDError::StorageConsistencyError(
                format!(
                    "Prefix {} contains a non-json file.",
                    prefix
                )
            ))
        } else {
            Ok(())
        }
    }

    async fn get_files_in_accepted_dir(&self) -> ABCDResult<Vec<Object>> {
        let prefix = {
            let gen_no = self.previous_gen_number_async().await? + 1;
            let gen_dir = format!("gen_{:03}", gen_no);
            format!(
                "{}/{}/accepted", 
                &self.particle_prefix, 
                gen_dir
            )
        };

        let objects = self.list_objects_v2(&prefix).await?;

        Self::assert_only_json(&objects, &prefix)?;

        Ok(objects)
    }

    async fn read_to_string(output: ABCDResult<GetObjectOutput>) -> ABCDResult<String> {
        let bytes: Bytes = output?
            .body
            .collect()
            .await
            .map_err(|e| 
                ABCDError::S3OperationError(format!("Empty byte stream: {}", e))
            )?
            .into_bytes();

        let string = std::str::from_utf8(&bytes).unwrap();
        Ok(string.into())
    }

    async fn get_versions(&self, prefix: &str) -> ABCDResult<ListObjectVersionsOutput> {
        let list_obj_ver = self
            .client
            .list_object_versions()
            .bucket(&self.bucket)
            .prefix(prefix)
            .send()
            .await?;

        if list_obj_ver.is_truncated {
            return Err(ABCDError::S3OperationError(format!(
                "Too many object verions - pagination not currently in use: {:?}",
                list_obj_ver
            )));
        }

        Ok(list_obj_ver)
    }

    async fn get_delete_marker_versions_with_pagination(&self, prefix: &str) -> ABCDResult< Vec<DeleteMarkerEntry> > {
        
        let mut acc: Vec<DeleteMarkerEntry> = Vec::new();

        async fn next_page(
            client: &Client,
            bucket: &str,
            prefix: &str,
            c_tok: Option<String>,
        ) -> ABCDResult<ListObjectVersionsOutput> {
            client
                .list_object_versions()
                .bucket(bucket)
                .prefix(prefix)
                .set_key_marker(c_tok)
                .send()
                .await
                .map_err(|e| e.into())
        }

        let mut c_token = None;
        loop {
            let list_output = next_page(&self.client, &self.bucket, prefix, c_token).await?;
            if let Some(mut items) = list_output.delete_markers {
                acc.append(&mut items);
            }

            c_token = list_output.key_marker;
            if c_token.is_none() {
                break;
            }
        }

        Ok(acc)
    
    }

    // async fn get_versionid_marker_versions_with_pagination(&self, prefix: &str) -> ABCDResult< Vec<ObjectVersion> > {
        
    //     let mut acc: Vec<ObjectVersion> = Vec::new();

    //     async fn next_page(
    //         client: &Client,
    //         bucket: &str,
    //         prefix: &str,
    //         c_tok: Option<String>,
    //     ) -> ABCDResult<ListObjectVersionsOutput> {
    //         client
    //             .list_object_versions()
    //             .bucket(bucket)
    //             .prefix(prefix)
    //             .set_key_marker(c_tok)
    //             .send()
    //             .await
    //             .map_err(|e| e.into())
    //     }

    //     let mut c_token = None;
    //     loop {
    //         let list_output = next_page(&self.client, &self.bucket, prefix, c_token).await?;
    //         if let Some(mut items) = list_output.version_id_marker {
    //             acc.append(&mut items);
    //         }

    //         c_token = list_output.key_marker;
    //         if c_token.is_none() {
    //             break;
    //         }
    //     }

    //     Ok(acc)
    
    // }

    async fn ensure_only_original_verions(&self, key: &str) -> ABCDResult<String> {
        let list_obj_ver = self.get_versions(key).await?;

        let mut versions = list_obj_ver.versions.unwrap_or_default();
        let delete_markers = list_obj_ver.delete_markers.unwrap_or_default();

        if !delete_markers.is_empty() {
            return Err(ABCDError::S3OperationError("Detected delete markers, which could result in stale data being read.".into()));
        }

        if versions.len() == 1 {
            if let Some(version) = versions.swap_remove(0).version_id {
                return Ok(version);
            } else {
                return Err(ABCDError::S3OperationError(format!(
                    "Only verion of {} has ID None",
                    key
                )));
            }
        }

        let oldest_version_id = if let Some(version) = versions.pop().and_then(|ov| ov.version_id) {
            version
        } else {
            return Err(ABCDError::S3OperationError(format!(
                "Oldest verion of {} has ID None",
                key
            )));
        };

        let vers_to_delete = versions.into_iter().filter_map(|ov| {
            if ov.key.is_some() && ov.version_id.is_some() {
                Some((ov.key.unwrap(), ov.version_id.unwrap()))
            } else {
                None
            }
        });
       
        let to_delete: Vec<ObjectIdentifier> = {
            vers_to_delete
                .map(|(key, id)| {
                    ObjectIdentifier::builder()
                        .set_version_id(Some(id))
                        .set_key(Some(key))
                        .build()
                })
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

    fn put_object_future(
        &self,
        key: &str,
        body: &str,
    ) -> impl Future<Output = ABCDResult<PutObjectOutput>> {
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
        let objects = self.list_objects_v2(&self.completed_prefix).await?;
        if !objects
            .iter()
            .filter_map(|o| o.key.as_ref())
            .any(|k| k.ends_with("abcd.init"))
        {
            return Err(ABCDError::StorageInitError);
        }
        let key_strings = objects.into_iter().filter_map(|obj| obj.key);
        let gen_number = key_strings
            .filter_map(|key| {
                let captures = self.completed_gen_re.captures(&key)?;
                captures
                    .name("gid")
                    .map(|m| m.as_str().parse::<u16>().ok())
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
        self.runtime.block_on(async {
            let prev_gen_no = self.previous_gen_number_async().await?;
            let object_key = {
                let prev_gen_file_name = format!("gen_{:03}.json", prev_gen_no);
                format!(
                    "{}/{}",
                    &self.completed_prefix,
                    prev_gen_file_name
                )
            };

            let version_id = self.ensure_only_original_verions(&object_key).await?;
            let obj_string = self
                .client
                .get_object()
                .bucket(&self.bucket)
                .key(&object_key)
                .version_id(version_id)
                .send()
                .map_err(Into::<ABCDError>::into)
                .then(Self::read_to_string)
                .await?;

            let gen: Generation<P> = serde_json::from_str(&obj_string)?;

            if gen.number == prev_gen_no {
                Ok(gen)
            } else {
                Err(ABCDError::StorageConsistencyError(format!(
                    "Expected gen number {} but got {}",
                    prev_gen_no, gen.number
                )))
            }
        })
    }

    fn save_particle<P: Serialize>(&self, w: &Particle<P>) -> ABCDResult<String> {
        self.runtime.block_on(async {
            let object_path = {
                let gen_file_dir = {
                    let gen_no = self.previous_gen_number_async().await? + 1;
                    format!("gen_{:03}", gen_no)
                };

                let particle_file_name = {
                    let file_uuid = Uuid::new_v4();
                    file_uuid.to_string() + ".json"
                };

                let status = 
                    if w.weight > 0.0 { "accepted" }
                    else { "rejected" };

                format!(
                    "{}/{}/{}/{}",
                    &self.particle_prefix,
                    &gen_file_dir,
                    &status,
                    particle_file_name,
                )
            };

            let pretty_json = serde_json::to_string_pretty(w)?;
            self.put_object_future(&object_path, &pretty_json).await?;

            Ok(object_path)
        })
    }

    fn num_accepted_particles(&self) -> ABCDResult<u32> {
        let files_in_folder = self
            .runtime
            .block_on(self.get_files_in_accepted_dir());

        match files_in_folder {
            Ok(files) => Ok(files.len().try_into()?),
            Err(e) => Err(e),
        }
    }

    fn load_accepted_particles<P>(&self) -> ABCDResult<Vec<Particle<P>>>
    where
        P: DeserializeOwned,
    {
        self.runtime.block_on(async {
            let object_names = self
                .get_files_in_accepted_dir()
                .await?
                .into_iter()
                .map(|t| t.key)
                .collect::<Option<Vec<String>>>()
                .ok_or_else(|| {
                    ABCDError::Other("failed to identify all particle file names".into())
                })?;

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
                            serde_json::from_str::<Particle<P>>(&s).map_err(|e| {
                                ABCDError::SerdeError(format!(
                                    "Failed to deserialise {}: {}",
                                    filename.clone(),
                                    e
                                ))
                            })
                        })
                    })
            });

            let joined = futures::future::join_all(particle_futures);
            let particles: Vec<Result<Particle<P>, ABCDError>> = joined.await;

            let result_of_vec: ABCDResult<Vec<Particle<P>>> = particles.into_iter().collect();
            result_of_vec
        })
    }

    fn save_new_gen<P: Serialize>(&self, gen: &Generation<P>) -> ABCDResult<()> {
        
        self.runtime.block_on(async {
            let expected_new_gen_number = self.previous_gen_number_async().await? + 1;
            if gen.number != expected_new_gen_number {
                return Err(ABCDError::StorageConsistencyError(format!(
                    "Asked to save gen {}, but was due to save {}",
                    &gen.number, &expected_new_gen_number
                )));
            }

            let object_path = {
                let object_name = format!("gen_{:03}.json", gen.number);

                format!(
                    "{}/{}", 
                    &self.completed_prefix, 
                    object_name
                )
            };

            //Test if the file is somehow already there
            let get_acl_output = self
                .client
                .get_object_acl()
                .bucket(&self.bucket)
                .key(&object_path)
                .send()
                .await;

                let json = &serde_json::to_string_pretty(gen)?;
                println!("JSON about to be placed into Gen file is: {:?}",&json);

            match get_acl_output {
                Err(SdkError::ServiceError {
                    err: GetObjectAclError { .. },
                    raw: _,
                }) => {
                    //This is good, means we're not writing over existing gen
                    self.put_object_future(&object_path, &serde_json::to_string_pretty(gen)?)
                        .await?;
                    Ok(())
                }
                _ => {
                    //This is bad, the file shouldn't exist before we've saved it!
                    Err(ABCDError::GenAlreadySaved(format!(
                        "Gen file already existed at {:?}",
                        object_path
                    )))
                }
            }
        })
    }

    fn num_rejected_particles(&self) -> ABCDResult<u64> {
        self.runtime.block_on(async {
            let prefix = {
                let current_gen = self.previous_gen_number_async().await? + 1;
                let gen_dir = format!("gen_{:03}", current_gen);

                format!(
                    "{}/{}/rejected",
                    &self.particle_prefix,
                    &gen_dir
                )
            };
            
            let objects = self.list_objects_v2(&prefix).await?;
            Self::assert_only_json(&objects, &prefix)?;

            Ok(cast::u64(objects.len()))
        })
    }
}