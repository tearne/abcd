use std::fmt::Debug;
use std::path::{Path, PathBuf};

// abc {
//     job {
//         replicates = 4
//         particles = 1000
//         generations = 50
//     }	
//     algorithm {
//         particle-retries = 100  <--- will we still want this?
//         particle-chunk-size = 1 <--- probably don't need it
//         tolerance-descent-percentile = 0.5
//         fewest-accepted-local-particles = 0 <--- gone
//     }
//     cluster {
            //Add the storage in here
//         system-name: ${CLUSTER_NAME}
//         max-particle-memory = 1000000
//         terminate-at-target-generation = false <--- perhaps keep?
//         futures-timeout = 90 days
//          mixing {
//              rate = 1 minutes
//              num-particles = 500
//             response-threshold = 5 seconds
//         }
//         size-reporting = 1 hour
//     }
//  }


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
    pub storage: Storage,
    pub job: Job,
    pub algorithm: Algorithm,
}
impl Config {
    pub fn from_path<P>(config_path: P) -> Self
    where
        P: AsRef<Path> + Debug,
    {
        let str = std::fs::read_to_string(config_path.as_ref())
            .unwrap_or_else(|e| panic!("Failed to load config from {:?}: {}", config_path, e));
        
        let config: Config = toml::from_str(&str).unwrap();
        log::info!("Loading config: {:#?}", config);
        config
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use super::Config;

     #[test]
    fn load_from_env_var() {
        envmnt::set("ABCDBucket", "s3://my-bucket");
        
        //TODO put in helper
        let mut d = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        d.push("resources/test/config_test.toml");

        let config = Config::from_path(d);
        //TODO Want to use TEST_BUCKET for other tests - but then don't want to show value
        //What do we do here - have two different toml files - thats whay I tried anyway.
        assert_eq!("s3://my-bucket", config.storage.get_path_string());
    }
}
