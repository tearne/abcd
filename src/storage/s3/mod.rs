#[cfg(test)]
mod tests;

use aws_sdk_s3::error::GetObjectAclError;
use aws_sdk_s3::model::{
    BucketVersioningStatus, Delete, Object, ObjectCannedAcl, ObjectIdentifier,
};
use aws_sdk_s3::output::{GetObjectOutput, ListObjectsV2Output, PutObjectOutput};
use aws_sdk_s3::{ByteStream, Client, Region, SdkError};
use bytes::Bytes;
use futures::{Future, FutureExt, TryFutureExt, TryStreamExt};
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
    gen_non_zero_re: Regex,
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

        instance
            .runtime
            .block_on(instance.assert_versioning_active())?;

        Ok(instance)
    }

    async fn assert_versioning_active(&self) -> ABCDResult<()> {
        let enabled = self
            .client
            .get_bucket_versioning()
            .bucket(&self.bucket)
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

    fn assert_only_json(objects:&Vec<Object>, prefix: &str) -> ABCDResult<()> {
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
                &self.prefix, 
                gen_dir
            )
        };

        let objects = self.list_objects_v2(&prefix).await?;

        Self::assert_only_json(&objects, &prefix)?;

        Ok(objects)
    }

    async fn read_to_string(output: ABCDResult<GetObjectOutput>) -> ABCDResult<String> {
        let bytes = output?
            .body
            .try_next()
            .await?
            .ok_or_else(|| ABCDError::S3OperationError("Empty byte stream".into()))?;

        let string = std::str::from_utf8(&bytes).unwrap();
        Ok(string.into())
    }

    async fn ensure_only_original_verions(&self, key: &str) -> ABCDResult<String> {
        let list_obj_ver = self
            .client
            .list_object_versions()
            .bucket(&self.bucket)
            .prefix(key)
            .send()
            .await?;

        if list_obj_ver.is_truncated {
            return Err(ABCDError::S3OperationError(format!(
                "Too many object verions - pagination not currently in use: {:?}",
                list_obj_ver
            )));
        }

        let mut versions = list_obj_ver.versions.unwrap_or_default();
        let delete_markers = list_obj_ver.delete_markers.unwrap_or_default();

        if versions.len() == 1 && delete_markers.is_empty() {
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
        let dms_to_delete = delete_markers.into_iter().filter_map(|ov| {
            if ov.key.is_some() && ov.version_id.is_some() {
                Some((ov.key.unwrap(), ov.version_id.unwrap()))
            } else {
                None
            }
        });
        let to_delete: Vec<ObjectIdentifier> = {
            vers_to_delete
                .chain(dms_to_delete)
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
        let objects = self.list_objects_v2(&self.prefix).await?;

        if !objects
            .iter()
            .filter_map(|o| o.key.as_ref())
            .any(|k| self.gen_zero_re.is_match(k))
        {
            return Err(ABCDError::StorageInitError);
        }

        let key_strings = objects.into_iter().filter_map(|obj| obj.key);

        let gen_number = key_strings
            .filter_map(|key| {
                let captures = self.gen_non_zero_re.captures(&key)?;
                let g1 = captures
                    .name("gid1")
                    .map(|m| m.as_str().parse::<u16>().ok())
                    .flatten();
                let g2 = captures
                    .name("gid2")
                    .map(|m| m.as_str().parse::<u16>().ok())
                    .flatten();

                match (g1, g2) {
                    (Some(gid_1), Some(gid_2)) => Some((key.clone(), gid_1, gid_2)),
                    _ => None,
                }
            })
            .map(|(key, gid_1, gid_2)| {
                if gid_1 != gid_2 {
                    Err(ABCDError::StorageConsistencyError(format!(
                        "Inconsistent path: {}",
                        &key
                    )))
                } else {
                    Ok(gid_1)
                }
            })
            .collect::<ABCDResult<Vec<u16>>>()?
            .into_iter()
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
                    &self.prefix,
                    gen_file_dir,
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
                let gen_dir = format!("gen_{:03}", gen.number);
                let object_name = format!("gen_{:03}.json", gen.number);

                format!(
                    "{}/{}/{}", 
                    &self.prefix, 
                    gen_dir, object_name
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
                    &self.prefix,
                    &gen_dir
                )
            };
            
            let objects = self.list_objects_v2(&prefix).await?;
            Self::assert_only_json(&objects, &prefix)?;

            Ok(cast::u64(objects.len()))
        })
    }
}