#[cfg(test)]
mod tests;

use aws_sdk_s3::Client;
use aws_sdk_s3::config::Region;
use aws_sdk_s3::operation::get_object::GetObjectOutput;
use aws_sdk_s3::operation::list_object_versions::ListObjectVersionsOutput;
use aws_sdk_s3::operation::list_objects_v2::ListObjectsV2Output;
use aws_sdk_s3::operation::put_object::PutObjectOutput;
use aws_sdk_s3::types::{ObjectIdentifier, Delete, BucketVersioningStatus, Object, ObjectCannedAcl};
use aws_smithy_http::byte_stream::ByteStream;
use aws_smithy_http::result::SdkError;
use bytes::Bytes;
use envmnt::{ExpandOptions, ExpansionType};
use futures::{Future, FutureExt, TryFutureExt};
use regex::Regex;
use serde::{de::DeserializeOwned, Serialize};
use std::convert::TryInto;
use std::fmt::Debug;
use tokio::runtime::Handle;

use super::Storage;
use crate::error::{ABCDErr, ABCDResult};
use crate::{Generation, Particle};
use tokio;
use uuid::Uuid;

pub struct S3System {
    pub bucket: String,
    client: Client,
    pub(super) handle: Handle,
    pub prefix: String,
    particle_prefix: String,
    completed_prefix: String,
    completed_gen_re: Regex,
}
impl S3System {
    pub fn new(bucket: &str, prefix: &str, handle: Handle) -> ABCDResult<Self> {
        if bucket.starts_with("s3://") {
            return Err(ABCDErr::SystemError(
                "Bucket in config shouldn't start with 's3://'.  Just provide the bucket name.".into()
            ));
        }

        // Expand bucket environment variables as appropriate
        let mut options = ExpandOptions::new();
        options.expansion_type = Some(ExpansionType::Unix);
        let bucket = envmnt::expand(&bucket, Some(options));
        let prefix = envmnt::expand(&prefix, Some(options));
        
        let client = {
            let config = handle.block_on(
                aws_config::from_env()
                    .region(Region::new("eu-west-1"))
                    .load(),
            );
            Client::new(&config)
        };

        let completed_prefix = format!("{}/completed", &prefix,);

        let particle_prefix = format!("{}/particles", &prefix);

        let completed_gen_re = {
            let string = format!(r#"^{}/completed/gen_(?P<gid>\d*).json"#, &prefix);
            Regex::new(&string)?
        };

        let instance = S3System {
            bucket,
            client,
            handle,
            prefix,
            particle_prefix,
            completed_prefix,
            completed_gen_re,
        };

        instance
            .handle
            .block_on(instance.assert_versioning_active())?;

        Ok(instance)
    }

    pub fn purge_all_versions_of_everything_in_prefix(&self) -> ABCDResult<()> {
        self.handle.block_on(async {
            self.assert_versioning_active().await?;
            let version_pages = self.get_versions(&self.prefix).await?;

            for page in version_pages {
                let mut object_identifiers = Vec::new();

                let object_versions = page.versions.unwrap_or_default();
                let delete_markers = page.delete_markers.unwrap_or_default();

                let it = delete_markers.into_iter().map(|item| {
                    ObjectIdentifier::builder()
                        .set_version_id(item.version_id)
                        .set_key(item.key)
                        .build()
                });
                object_identifiers.extend(it);

                let it = object_versions.into_iter().map(|item| {
                    ObjectIdentifier::builder()
                        .set_version_id(item.version_id)
                        .set_key(item.key)
                        .build()
                });
                object_identifiers.extend(it);

                if !object_identifiers.is_empty() {
                    log::info!("Deleting {} identifiers", object_identifiers.len());
                    self.client
                        .delete_objects()
                        .bucket(&self.bucket)
                        .delete(
                            Delete::builder()
                                .set_objects(Some(object_identifiers))
                                .build(),
                        )
                        .send()
                        .await?;
                } else {
                    log::info!("Nothing to delete")
                }
            }

            ABCDResult::Ok(())
        })
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
            Err(ABCDErr::InfrastructureError(
                "S3 bucket versioning must be enabled".into(),
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

            c_token = list_output.next_continuation_token().map(str::to_string);

            if let Some(mut items) = list_output.contents {
                acc.append(&mut items);
            }

            if c_token.is_none() {
                break;
            }
        }

        Ok(acc)
    }

    fn assert_only_json(objects: &[Object], prefix: &str) -> ABCDResult<()> {
        let is_not_json_file = |o: &Object| {
            o.key
                .as_ref()
                .map(|k| !k.ends_with(".json"))
                .unwrap_or(true)
        };

        if objects.iter().any(is_not_json_file) {
            Err(ABCDErr::SystemError(format!(
                "Prefix {} contains a non-json file.",
                prefix
            )))
        } else {
            Ok(())
        }
    }

    async fn get_files_in_accepted_dir(&self) -> ABCDResult<Vec<Object>> {
        let prefix = {
            let gen_no = self.previous_gen_number_async().await? + 1;
            let gen_dir = format!("gen_{:03}", gen_no);
            format!("{}/{}/accepted", &self.particle_prefix, gen_dir)
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
            .map_err(|e| ABCDErr::InfrastructureError(format!("Empty S3 byte stream: {}", e)))?
            .into_bytes();

        let string = std::str::from_utf8(&bytes)
            .map_err(|e|ABCDErr::InfrastructureError(e.to_string()))?;
        Ok(string.into())
    }

    async fn get_versions(&self, prefix: &str) -> ABCDResult<Vec<ListObjectVersionsOutput>> {
        async fn next_page(
            client: &Client,
            bucket: &str,
            prefix: &str,
            next_key: Option<String>,
            next_version: Option<String>,
        ) -> ABCDResult<ListObjectVersionsOutput> {
            client
                .list_object_versions()
                .bucket(bucket)
                .prefix(prefix)
                .set_key_marker(next_key)
                .set_version_id_marker(next_version)
                .send()
                .await
                .map_err(|e| e.into())
        }

        let mut next_key = None;
        let mut next_version = None;

        let mut acc: Vec<ListObjectVersionsOutput> = Vec::new();

        loop {
            let out = next_page(&self.client, &self.bucket, prefix, next_key, next_version).await?;

            next_key = out.next_key_marker.clone().map(String::from);
            next_version = out.next_version_id_marker.clone().map(String::from);

            acc.push(out);

            log::debug!("Found {} page(s) of version identifiers.", acc.len());

            if next_key.is_none() && next_version.is_none() {
                break;
            }
        }

        Ok(acc)
    }

    async fn ensure_only_original_versions(&self, key: &str) -> ABCDResult<String> {
        let mut version_pages = self.get_versions(key).await?;
        let first_page = version_pages.swap_remove(0);

        let mut versions = first_page.versions.unwrap_or_default();
        let delete_markers = first_page.delete_markers.unwrap_or_default();

        if !delete_markers.is_empty() {
            return Err(ABCDErr::InfrastructureError(
                "Detected S3 delete markers, which could result in stale data being read.".into(),
            ));
        }

        if versions.len() == 1 {
            if let Some(version) = versions.swap_remove(0).version_id {
                return Ok(version);
            } else {
                return Err(ABCDErr::InfrastructureError(format!(
                    "In S3, the only version of {} has ID None",
                    key
                )));
            }
        }

        let oldest_version_id = if let Some(version) = versions.pop().and_then(|ov| ov.version_id) {
            version
        } else {
            return Err(ABCDErr::InfrastructureError(format!(
                "In S3, the oldest version of {} has ID None",
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
            .map_err(Into::<ABCDErr>::into)
    }

    async fn previous_gen_number_async(&self) -> ABCDResult<u16> {
        let objects = self.list_objects_v2(&self.completed_prefix).await?;
        if !objects
            .iter()
            .filter_map(|o| o.key.as_ref())
            .any(|k| k.ends_with("abcd.init"))
        {
            return Err(ABCDErr::InfrastructureError(
                format!("abcd.init marker not found in S3 at bucket {}, prefix {}", self.bucket, self.completed_prefix),
            ));
        }
        let key_strings = objects.into_iter().filter_map(|obj| obj.key);
        let gen_number = key_strings
            .filter_map(|key| {
                let captures = self.completed_gen_re.captures(&key)?;
                captures
                    .name("gid")
                    .and_then(|m| m.as_str().parse::<u16>().ok())
            })
            .max()
            .unwrap_or(0);

        Ok(gen_number)
    }
}
impl Storage for S3System {
    fn previous_gen_number(&self) -> ABCDResult<u16> {
        self.handle.block_on(self.previous_gen_number_async())
    }

    fn load_previous_gen<P>(&self) -> ABCDResult<Generation<P>>
    where
        P: DeserializeOwned + Debug,
    {
        self.handle.block_on(async {
            let prev_gen_no = self.previous_gen_number_async().await?;
            let object_key = {
                let prev_gen_file_name = format!("gen_{:03}.json", prev_gen_no);
                format!("{}/{}", &self.completed_prefix, prev_gen_file_name)
            };

            let version_id = self.ensure_only_original_versions(&object_key).await?;
            let obj_string = self
                .client
                .get_object()
                .bucket(&self.bucket)
                .key(&object_key)
                .version_id(version_id)
                .send()
                .map_err(Into::<ABCDErr>::into)
                .then(Self::read_to_string)
                .await?;

            let gen: Generation<P> = serde_json::from_str(&obj_string)?;

            if gen.number == prev_gen_no {
                Ok(gen)
            } else {
                Err(ABCDErr::SystemError(format!(
                    "Expected object to contain gen number {} but upon deserialisation found {}",
                    prev_gen_no, gen.number
                )))
            }
        })
    }

    fn save_particle<P: Serialize>(&self, w: &Particle<P>, gen_num: u16) -> ABCDResult<String> {
        self.handle.block_on(async {
            let object_path = {
                let gen_file_dir = format!("gen_{:03}", gen_num);

                let particle_file_name = {
                    let file_uuid = Uuid::new_v4();
                    file_uuid.to_string() + ".json"
                };

                let status = if w.weight > 0.0 {
                    "accepted"
                } else {
                    "rejected"
                };

                format!(
                    "{}/{}/{}/{}",
                    &self.particle_prefix, &gen_file_dir, &status, particle_file_name,
                )
            };

            let pretty_json = serde_json::to_string_pretty(w)?;

           
            let building_generation = self.previous_gen_number_async().await? + 1;
            if gen_num != building_generation {
                // This check will only stop obvious errors, it doesn't  protect against the situation when a
                // generation is flushed at the same time as the particle is saved.  However, this doesn't 
                // really matter, as the worst that would happen is that the particle is saved to the previous 
                // generation after the flush has taken place.
                return Err(ABCDErr::SystemError(format!(
                    "You tried to save a particle against generation {} but the building generation is {}",
                    gen_num,
                    building_generation
                )))
            };

            self.put_object_future(&object_path, &pretty_json).await?;

            Ok(object_path)
        })
    }

    fn num_accepted_particles(&self) -> ABCDResult<u32> {
        let files_in_folder = self.handle.block_on(self.get_files_in_accepted_dir());

        match files_in_folder {
            Ok(files) => Ok(files.len().try_into()?),
            Err(e) => Err(e),
        }
    }

    fn load_accepted_particles<P>(&self) -> ABCDResult<Vec<Particle<P>>>
    where
        P: DeserializeOwned,
    {
        self.handle.block_on(async {
            let object_names = self
                .get_files_in_accepted_dir()
                .await?
                .into_iter()
                .map(|t| t.key)
                .collect::<Option<Vec<String>>>()
                .ok_or_else(|| {
                    ABCDErr::InfrastructureError(
                        "In S3, failed to identify all particle file names".into(),
                    )
                })?;

            let particle_futures = object_names.into_iter().map(|filename| {
                self.client
                    .get_object()
                    .bucket(&self.bucket)
                    .key(&filename)
                    .send()
                    .map_err(Into::<ABCDErr>::into)
                    .then(Self::read_to_string)
                    .map(|res| {
                        res.and_then(move |s| {
                            serde_json::from_str::<Particle<P>>(&s).map_err(|e| {
                                ABCDErr::SystemError(format!(
                                    "After load from S3, Serde failed to deserialise {}: {}",
                                    filename.clone(),
                                    e
                                ))
                            })
                        })
                    })
            });

            let joined = futures::future::join_all(particle_futures);
            let particles: Vec<Result<Particle<P>, ABCDErr>> = joined.await;

            let result_of_vec: ABCDResult<Vec<Particle<P>>> = particles.into_iter().collect();
            result_of_vec
        })
    }

    fn save_new_gen<P: Serialize>(&self, gen: &Generation<P>) -> ABCDResult<()> {
        self.handle.block_on(async {
            let expected_new_gen_number = self.previous_gen_number_async().await? + 1;
            if gen.number != expected_new_gen_number {
                return Err(ABCDErr::SystemError(format!(
                    "Asked to save gen {} to S3, but was expecting to save {}",
                    &gen.number, &expected_new_gen_number
                )));
            }

            let object_path = {
                let object_name = format!("gen_{:03}.json", gen.number);

                format!("{}/{}", &self.completed_prefix, object_name)
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
                //TODO more explicit check that file doesn't exist than asking for ACL and failing
                Err(SdkError::ServiceError(_)) => {
                    //This is good, means we're not writing over existing gen
                    self.put_object_future(&object_path, &serde_json::to_string_pretty(gen)?)
                        .await?;
                    Ok(())
                },
                _ => {
                    //This is bad, the file shouldn't exist before we've saved it!
                    Err(ABCDErr::SystemError(format!(
                        "Gen file already existed in S3 at {:?}",
                        object_path
                    )))
                }
            }
        })
    }

    fn num_rejected_particles(&self) -> ABCDResult<u64> {
        self.handle.block_on(async {
            let prefix = {
                let current_gen = self.previous_gen_number_async().await? + 1;
                let gen_dir = format!("gen_{:03}", current_gen);

                format!("{}/{}/rejected", &self.particle_prefix, &gen_dir)
            };

            let objects = self.list_objects_v2(&prefix).await?;
            Self::assert_only_json(&objects, &prefix)?;

            Ok(cast::u64(objects.len()))
        })
    }
}
