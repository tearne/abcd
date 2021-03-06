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
pub struct Storage {
    pub location: String,
    pub kind: String,
}
impl Storage {
    pub fn get_path_string(&self) -> String {
        match self.kind.as_str() {
            "s3" => format!("s3://{}", self.location),
            "envvar" => {
                println!(" --> {}", &self.location);
                envmnt::get_or_panic(&self.location)
            },
            _ => unimplemented!(),
        }
    }
}

#[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
pub struct Config {
    storage: Storage,
}
impl Config {
    pub fn from_path<P>(config_path: P) -> Self
    where
        P: AsRef<Path> + Debug,
    {
        let str = std::fs::read_to_string(config_path.as_ref())
            .unwrap_or_else(|e| panic!("Failed to load config from {:?}: {}", config_path, e));
        
        log::info!("Loading config: {}", str);
        let config: Config = toml::from_str(&str).unwrap();
        config
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use super::Config;

     #[test]
    fn load_from_env_var() {
        if !envmnt::exists("ABCDBucket") {
            envmnt::set("ABCDBucket", "s3://my-bucket");
        }
        
        //TODO put in helper
        let mut d = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        d.push("resources/test/config.toml");

        let config = Config::from_path(d);

        assert_eq!("s3://my-bucket", config.storage.get_path_string());
    }
}
