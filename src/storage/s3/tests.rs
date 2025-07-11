use std::{fs::DirEntry, path::Path};

use envmnt::{ExpandOptions, ExpansionType};
use futures::TryStreamExt;
use tokio::runtime::Runtime;

use crate::{
    storage::{
        config::StorageConfig,
        test_helper::{gen_002, make_dummy_generation, DummyParams},
    },
    test_helper::test_data_path,
};

use super::*;

struct StorageTestHelper {
    bucket: String,
    prefix: String,
    delete_prefix_on_drop: bool,
    client: Client,
    runtime: Runtime,
}
impl StorageTestHelper {
    pub fn new(prefix: &str, delete_prefix_on_drop: bool) -> Self {
        if !envmnt::exists("TEST_BUCKET") {
            panic!(
                "You need to set the environment variable 'TEST_BUCKET' before running this test."
            );
        }

        // Expand bucket environment variables as appropriate
        let mut options = ExpandOptions::new();
        options.expansion_type = Some(ExpansionType::Unix);
        let bucket = envmnt::expand("${TEST_BUCKET}", Some(options));
        let prefix = envmnt::expand(prefix, Some(options));

        let runtime = Runtime::new().unwrap();

        let client = {
            let config = runtime.block_on(
                aws_config::from_env()
                    .region(Region::new("eu-west-1"))
                    .load(),
            );

            Client::new(&config)
        };

        let instance = StorageTestHelper {
            bucket,
            prefix,
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

        let uploader = |de: &DirEntry| {
            let absolute_path = de.path();
            let stripped_path = absolute_path.strip_prefix(abs_project_path).unwrap();
            let object_name = prefix.join(stripped_path).to_string_lossy().into_owned();
            let file_contents = std::fs::read_to_string(&absolute_path).unwrap();
            self.put_object(&object_name, &file_contents);
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
        self.runtime.block_on(async {
            let bytes = self
                .client
                .get_object()
                .bucket(&self.bucket)
                .key(key)
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

    #[allow(dead_code)]
    fn list_objects_under(&self, sub_prefix: Option<&str>) -> Vec<Object> {
        let prefix = sub_prefix
            .map(|p| format!("{}/{}", self.prefix, p))
            .unwrap_or_else(|| self.prefix.clone());

        let response = self.runtime.block_on({
            self.client
                .list_objects_v2()
                .bucket(&self.bucket)
                .prefix(prefix)
                .send()
        });

        let response = response.expect("Expected list objects response");
        assert!(response.continuation_token.is_none());
        response.contents.unwrap_or_default()
    }

    fn delete_prefix_recursively(&self) {
        if self.delete_prefix_on_drop {
            // if self
            //     .list_objects_under(None)
            //     .into_iter()
            //     .map(|o| ObjectIdentifier::builder().set_key(o.key).build())
            //     .next()
            //     .is_none()
            // {
            //     return;
            // }

            self.runtime.block_on(async {
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

                let mut acc_version_pages: Vec<ListObjectVersionsOutput> = Vec::new();

                loop {
                    let out = next_page(
                        &self.client,
                        &self.bucket,
                        &self.prefix,
                        next_key,
                        next_version,
                    )
                    .await
                    .unwrap();

                    next_key = out.next_key_marker.clone().map(String::from);
                    next_version = out.next_version_id_marker.clone().map(String::from);

                    acc_version_pages.push(out);

                    if next_key.is_none() && next_version.is_none() {
                        break;
                    }
                }

                for page in acc_version_pages {
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
                    } else {
                        log::info!("Nothing to delete")
                    }
                }

                // let _remaining = self
                //     .client
                //     .list_objects_v2()
                //     .bucket(&self.bucket)
                //     .prefix(&self.prefix)
                //     .send()
                //     .await
                //     .unwrap();
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

fn build_instance(helper: &StorageTestHelper) -> S3System {
    S3System::new(
        &helper.bucket,
        &helper.prefix,
        helper.runtime.handle().clone()
    ).expect("Failed to build storage instance")
}

#[test]
fn test_previous_gen_num_two() {
    let helper = StorageTestHelper::new("test_previous_gen_num_two", true);
    let instance = build_instance(&helper);

    helper.put_recursive("resources/test/storage/normal");

    assert_eq!(2, instance.previous_gen_number().unwrap());
}

#[test]
fn test_previous_gen_num_zero() {
    let helper = StorageTestHelper::new("test_previous_gen_num_zero", true);
    let instance = build_instance(&helper);

    helper.put_recursive("resources/test/storage/gen0");

    assert_eq!(0, instance.previous_gen_number().unwrap());
}

#[test]
fn test_restore_overwritten_gen() {
    let helper = StorageTestHelper::new("test_restore_overwritten_gen", true);
    let instance = build_instance(&helper);

    helper.put_recursive("resources/test/storage/normal");

    //Check this key already exists
    let key = &format!("{}/{}", &helper.prefix, "completed/gen_002.json");
    assert!(
        !helper.get_object(key).is_empty(),
        "expected key to exist so it could be overwritten"
    );

    //Overwrite it, as though it was uploaded concurrently by another node.
    helper.put_object(key, "Contents of an overwritten gen file");

    let loaded: Generation<DummyParams> = instance.load_previous_gen().unwrap();

    assert_eq!(gen_002(), loaded);
}

#[test]
fn test_load_previous_gen() {
    let helper = StorageTestHelper::new("test_load_previous_gen", true);
    let instance = build_instance(&helper);

    helper.put_recursive("resources/test/storage/normal");

    let result = instance.load_previous_gen::<DummyParams>().unwrap();

    assert_eq!(gen_002(), result);
}

#[test]
fn test_exception_if_save_without_init() {
    let helper = StorageTestHelper::new("test_exception_if_save_without_init", true);
    let instance = build_instance(&helper);

    helper.put_recursive("resources/test/storage/empty");

    let w1 = Particle {
        parameters: DummyParams::new(1, 2.),
        score: 100.0,
        weight: 1.234,
    };

    let result = instance.save_particle(&w1, 1);

    match result {
        Err(ABCDErr::InfrastructureError(_)) => (),
        _ => panic!("Expected exception"),
    }
}

#[test]
fn test_exception_if_save_particle_to_wrong_gen_num() {
    //Note, this is only a thin veil of security - it's still possible to save to a stale
    // generation if the gen gets flushed between the time the check is performed and the
    // time the save actually takes place.  But that wont really matter, since the flush
    // has already happened

    let helper = StorageTestHelper::new("test_exception_if_save_particle_to_wrong_gen_num", true);
    let instance = build_instance(&helper);

    helper.put_recursive("resources/test/storage/normal");

    let particle = Particle {
        parameters: DummyParams::new(1, 2.),
        score: 100.0,
        weight: 1.234,
    };

    match instance.save_particle(&particle, 2) {
        Err(ABCDErr::SystemError(_)) => (),
        Err(e) => panic!("Expected SystemErr, got: {}", e),
        Ok(_) => panic!("Expected SystemErr, got Ok"),
    }

    match instance.save_particle(&particle, 4) {
        Err(ABCDErr::SystemError(_)) => (),
        Err(e) => panic!("Expected SystemErr, got: {}", e),
        Ok(_) => panic!("Expected SystemErr, got Ok"),
    }
}

#[test]
fn test_save_particle() {
    let helper = StorageTestHelper::new("test_save_particle", true);
    let instance = build_instance(&helper);

    helper.put_recursive("resources/test/storage/normal");

    let particle = Particle {
        parameters: DummyParams::new(1, 2.),
        score: 100.0,
        weight: 1.234,
    };

    let save_path = instance.save_particle(&particle, 3).unwrap();
    assert!(!save_path.contains("rejected") && save_path.contains("accepted"));

    let loaded: Particle<DummyParams> =
        serde_json::from_str(&helper.get_object(&save_path)).unwrap();
    assert_eq!(particle, loaded);

    assert_eq!(
        2, // Only the two rejected particles that already existed
        instance.num_rejected_particles().unwrap()
    );
}

#[test]
fn test_save_particle_zero_weight() {
    let helper = StorageTestHelper::new("test_save_particle_zero_weight", true);
    let instance = build_instance(&helper);

    helper.put_recursive("resources/test/storage/normal");

    let zero_wt_particle = Particle {
        parameters: DummyParams::new(1, 2.),
        score: 100.0,
        weight: 0.0,
    };

    let save_path = instance.save_particle(&zero_wt_particle, 3).unwrap();

    assert!(save_path.contains("rejected") && !save_path.contains("accepted"));

    let loaded: Particle<DummyParams> =
        serde_json::from_str(&helper.get_object(&save_path)).unwrap();

    assert_eq!(zero_wt_particle, loaded);
}

#[test]
fn test_exception_if_accepted_contains_imposter() {
    let helper = StorageTestHelper::new("test_exception_if_accepted_contains_imposter", true);
    let instance = build_instance(&helper);

    helper.put_recursive("resources/test/storage/accepted_imposter_none_rejected");

    match instance.num_accepted_particles() {
        Err(ABCDErr::SystemError(_)) => (),
        Err(e) => panic!("Expected SystemErr, got: {}", e),
        Ok(_) => panic!("Expected SystemErr, got Ok"),
    }

    match instance.load_accepted_particles::<DummyParams>() {
        Err(ABCDErr::SystemError(_)) => (),
        Err(e) => panic!("Expected SystemErr, got: {}", e),
        Ok(_) => panic!("Expected SystemErr, got Ok"),
    }
}

#[test]
fn test_exception_if_rejected_contains_imposter() {
    let helper = StorageTestHelper::new("test_exception_if_rejected_contains_imposter", true);
    let instance = build_instance(&helper);

    helper.put_recursive("resources/test/storage/rejected_imposter_none_accepted");

    match instance.num_rejected_particles() {
        Err(ABCDErr::SystemError(_)) => (),
        Err(e) => panic!("Expected SystemErr, got: {}", e),
        Ok(_) => panic!("Expected SystemErr, got Ok"),
    }
}

#[test]
fn test_exception_if_save_inconsistent_gen_number() {
    let helper = StorageTestHelper::new("test_exception_saving_inconsistent_gen_number", true);
    let instance = build_instance(&helper);

    helper.put_recursive("resources/test/storage/normal");

    let dummy_generation = make_dummy_generation(999);

    let result = instance.save_new_gen(&dummy_generation);

    match result {
        Err(ABCDErr::SystemError(_)) => (),
        Err(e) => panic!("Expected SystemErr, got: {}", e),
        Ok(_) => panic!("Expected SystemErr, got Ok"),
    }
}

#[test]
fn test_exception_if_save_gen_which_already_exists() {
    let helper = StorageTestHelper::new("test_exception_if_save_gen_which_already_exists", true);
    let instance = build_instance(&helper);

    helper.put_recursive("resources/test/storage/normal");

    let dummy_generation = make_dummy_generation(2);

    let result = instance.save_new_gen(&dummy_generation);

    match result {
        Err(ABCDErr::SystemError(_)) => (),
        Err(e) => panic!("Expected SystemErr, got: {}", e),
        Ok(_) => panic!("Expected SystemErr, got Ok"),
    }
}

#[test]
fn test_no_accepted_particles() {
    let helper = StorageTestHelper::new("test_no_accepted_particles", true);
    let instance = build_instance(&helper);

    helper.put_recursive("resources/test/storage/rejected_imposter_none_accepted");

    assert_eq!(0, instance.num_accepted_particles().unwrap());
}

#[test]
fn test_num_accepted_particles() {
    let helper = StorageTestHelper::new("test_num_accepted_particles", true);
    let instance = build_instance(&helper);

    helper.put_recursive("resources/test/storage/normal");

    assert_eq!(2, instance.num_accepted_particles().unwrap());

    //Now upload more than a 'pages worth', to check pagination
    (0..1009).for_each(|i| {
        let key = &format!("{}/particles/gen_003/accepted/{}.json", &helper.prefix, i);
        helper.put_object(key, "");
    });

    let result = instance.num_accepted_particles().unwrap();

    assert_eq!(
        1011, // 1009 + 2 already in the orignal folder
        result
    );
}

#[test]
fn test_load_accepted_particles() {
    let helper = StorageTestHelper::new("test_load_accepted_particles", true);
    let instance = build_instance(&helper);

    helper.put_recursive("resources/test/storage/normal");

    let mut expected = {
        let w1 = Particle {
            parameters: DummyParams::new(1, 2.),
            score: 100.0,
            weight: 1.234,
        };

        let w2 = Particle {
            parameters: DummyParams::new(3, 4.),
            score: 200.0,
            weight: 1.567,
        };

        vec![w1, w2]
    };

    let mut result: Vec<Particle<DummyParams>> = instance.load_accepted_particles().unwrap();

    //Sort by weight for easy comparison
    expected.sort_by(|a, b| a.weight.partial_cmp(&b.weight).unwrap());
    result.sort_by(|a, b| a.weight.partial_cmp(&b.weight).unwrap());

    //Note that the zero weighted particle (contained in rejected folder) is not loaded
    assert_eq!(expected, result);
}

#[test]
fn test_no_rejected_particles() {
    let helper = StorageTestHelper::new("test_no_rejected_particles", true);
    let instance = build_instance(&helper);

    helper.put_recursive("resources/test/storage/accepted_imposter_none_rejected");

    assert_eq!(0, instance.num_rejected_particles().unwrap());
}

#[test]
fn test_num_rejected_particles() {
    let helper = StorageTestHelper::new("test_num_rejected_particles", true);
    let instance = build_instance(&helper);

    helper.put_recursive("resources/test/storage/normal");
    assert_eq!(2, instance.num_rejected_particles().unwrap());

    //Now upload more than a 'pages worth', to check pagination
    (0..1009).for_each(|i| {
        helper.put_object(
            &format!("{}/particles/gen_003/rejected/{}.json", &helper.prefix, i),
            "",
        );
    });

    assert_eq!(
        1011, // 1009 + 2 already in the orignal folder
        instance.num_rejected_particles().unwrap()
    );
}

#[test]
fn test_save_generation() {
    let helper = StorageTestHelper::new("test_save_generation", true);
    let instance = build_instance(&helper);

    helper.put_recursive("resources/test/storage/normal");

    let gen_number = 3;
    let dummy_generation = make_dummy_generation(gen_number);

    instance
        .save_new_gen(&dummy_generation)
        .expect("Expected successful save");

    let expected = dummy_generation;

    //Manually load what was saved to S3 to check
    let actual: Generation<DummyParams> = serde_json::from_str(&helper.get_object(&format!(
        "{}/completed/gen_{:03}.json",
        &instance.prefix, gen_number,
    )))
    .unwrap();

    assert_eq!(expected, actual);
}

#[test]
fn test_purge_all_versions_of_everything() {
    let helper = StorageTestHelper::new("test_purge_all_versions_of_everything", true);
    let instance = build_instance(&helper);

    //Put a bunch of files
    helper.put_recursive("resources/test/storage/normal");
    helper.put_recursive("resources/test/storage/normal");

    instance
        .purge_all_versions_of_everything_in_prefix()
        .unwrap();

    //There should be no versions of anything left
    let mut pages = helper
        .runtime
        .block_on(async { instance.get_versions(&instance.prefix).await })
        .unwrap();
    assert!(pages.len() == 1);

    let versions = pages.swap_remove(0);
    assert!(versions.versions().is_none());
    assert!(versions.delete_markers().is_none());
}
