use std::fmt::Debug;
use std::path::{Path, PathBuf};

// #[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
// pub struct SystemParams {
//     input_data_root: PathBuf //TODO can we make this a Path? lifetime seems to clash
// }
// impl SystemParams {
//     // pub fn absoluteify_root_path(&mut self, config_path: impl AsRef<Path>) {
//     //     if !self.input_data_root.starts_with("/") {
//     //         self.input_data_root = config_path
//     //             .as_ref()
//     //             .parent()
//     //             .unwrap()
//     //             .join(self.input_data_root.as_path())
//     //     };
//     // }

// }

#[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
pub struct Config {
    //pub system_params: SystemParams
    bucket: String,
}
impl Config {
    pub fn from_path<P>(config_path: P) -> Self
    where
        P: AsRef<Path> + Debug,
    {
        let str = std::fs::read_to_string(config_path.as_ref())
            .unwrap_or_else(|e| panic!("Failed to load config from {:?}: {}", config_path, e));
        let mut config: Self = toml::from_str(&str).unwrap();
        config.system_params.absoluteify_root_path(config_path);
        config
    }

    pub fn resolve(&mut self){
        if self.bucket.starts_with('$') {
            let var_name = self.bucket;
            //TODO test panics if can't find it
            self.bucket = envmnt::get(var_name);
        }
    }
}

#[cfg(test)]
mod tests {
    fn error_if_dont_call_resolve_when_using_env_var(){ todo!() }

    fn load_from_env_var() {
        if !envmnt::exists("$ABCDBucket") {
            envmnt::set("$ABCDBucket", "info");
        }

        let mut d = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        d.push("resources/test/config.toml");
        let mut config = Config::from_path(d);
        config.resolve();  //Badly named function which resolves env vars

        assert_eq!("s3://my-bucket", config.bucket);
    }
}
