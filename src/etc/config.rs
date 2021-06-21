// use std::fmt::Debug;
// use std::path::{Path, PathBuf};

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

// #[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
// pub struct Config {
//     pub system_params: SystemParams
// }
// impl Config {
//     pub fn from_path<P>(config_path: P) -> Self
//     where
//         P: AsRef<Path> + Debug,
//     {
//         let str = std::fs::read_to_string(config_path.as_ref())
//             .unwrap_or_else(|e| panic!("Failed to load config from {:?}: {}", config_path, e));
//         let mut config: Self = toml::from_str(&str).unwrap();
//         config.system_params.absoluteify_root_path(config_path);
//         config
//     }
// }
