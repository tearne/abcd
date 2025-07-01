use std::{borrow::Cow, fmt::Debug};

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
}
