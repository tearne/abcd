use std::borrow::Cow;

use envmnt::{ExpandOptions, ExpansionType};
use tokio::runtime::Handle;

use crate::error::{ABCDErr, ABCDResult};

use super::s3::S3System;

#[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
pub struct StorageConfig<'a> {
    pub bucket: Cow<'a, str>, 
    pub prefix: Cow<'a, str>,
}
impl<'a> StorageConfig<'a> {

    pub fn new<P: Into<Cow<'a, str>>>(bucket: P, prefix: P) -> Self {
        Self {
            bucket: bucket.into(),
            prefix: prefix.into()
        }
    }
    
    pub fn build(&self, handle: Handle) -> ABCDResult<S3System> {
        if self.bucket.starts_with("s3://") {
            return Err(ABCDErr::SystemError(
                "Bucket in config shouldn't start with 's3://'.  Just provide the bucket name.".into()
            ));
        }

        // Expand bucket environment variables as appropriate
        let mut options = ExpandOptions::new();
        options.expansion_type = Some(ExpansionType::Unix);
        let bucket = envmnt::expand(&self.bucket, Some(options));
        let prefix = envmnt::expand(&self.prefix, Some(options));

        S3System::new(bucket, prefix, handle)
    }
}

#[cfg(test)]
mod tests {
    use std::env;

    use tokio::runtime::Runtime;

    use super::StorageConfig;
    use crate::error::ABCDResult;

    #[test]
    fn build_s3_storage_properties_from_config_expanding_env_var() -> ABCDResult<()> {
        if !envmnt::exists("TEST_BUCKET") {
            panic!(
                " *** You need to set the environment variable 'TEST_BUCKET' before running this test."
            );
        }
        if !envmnt::exists("TEST_PREFIX") {
            panic!(
                " *** You need to set the environment variable 'TEST_PREFIX' before running this test."
            );
        }

        let storage_config = StorageConfig {
            bucket: "$TEST_BUCKET".into(),
            prefix: "$TEST_PREFIX".into(),
        };

        let runtime = Runtime::new()?;
        let handle = runtime.handle();
        let storage = storage_config.build(handle.clone())?;

        assert_eq!(env::var("TEST_BUCKET").unwrap(), storage.bucket);
        assert_eq!(env::var("TEST_PREFIX").unwrap(), storage.prefix);

        Ok(())
    }
}
