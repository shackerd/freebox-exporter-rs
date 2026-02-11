use sections::{ApiConfiguration, CoreConfiguration, LogConfiguration, CapabilitiesConfiguration};
use serde::Deserialize;
use std::{
    fs::{self},
    path::Path,
};
use tokio::{fs::File, io::AsyncReadExt};

use crate::core::configuration::sections::PoliciesConfiguration;

pub mod sections;
pub mod tests;
#[derive(Deserialize, Clone, Debug)]
pub struct Configuration {
    pub api: ApiConfiguration,
    pub metrics: CapabilitiesConfiguration,
    pub core: CoreConfiguration,
    pub log: LogConfiguration,
    pub policies: PoliciesConfiguration,
}

impl Configuration {
    pub fn assert_data_dir_permissions(&self) -> Result<(), &str> {
        let data_dir = self.core.data_directory.to_owned().unwrap();

        let path = Path::new(&data_dir);

        if !path.try_exists().expect("Access is denied") {
            return Err("data dir does not exist");
        }

        let permissions = fs::metadata(path)
            .expect("cannot read metadata")
            .permissions();

        if permissions.readonly() {
            return Err("data_dir cannot be readonly");
        }

        Ok(())
    }

    pub fn assert_metrics_prefix_is_not_empty(&self) -> Result<(), ()> {
        self.metrics.prefix.clone().map_or_else(
            || Err(()),
            |v| match v.trim() {
                "" => Err(()),
                _ => Ok(()),
            },
        )
    }
}

pub async fn get_configuration(
    file_path: String,
) -> Result<Configuration, Box<dyn std::error::Error + Send + Sync>> {
    let path = Path::new(&file_path);

    if !path.exists() {
        panic!("Configuration file is missing");
    }

    let mut file = File::open(path).await?;
    let mut buffer = vec![];

    file.read_to_end(&mut buffer).await?;

    let result = String::from_utf8(buffer)?;

    match toml::from_str::<Configuration>(&result) {
        Ok(c) => {
            if let Err(e) = assert_configuration_is_valid(&c) {
                return Err(e);
            }

            return Ok(c);
        }
        Err(e) => {
            println!("{e:#?}");
            panic!("Configuration file is corrupted");
        }
    }
}

fn assert_configuration_is_valid(
    c: &Configuration,
) -> Result<Configuration, Box<dyn std::error::Error + Send + Sync>> {
    let permissions = c.assert_data_dir_permissions();

    if let Err(e) = permissions {
        println!("{e:#?}");
        return Err("data dir does not exist or access is denied".into());
    }
    let prefix = c.assert_metrics_prefix_is_not_empty();

    if let Err(e) = prefix {
        println!("{e:#?}");
        return Err("metrics prefix cannot be empty".into());
    }

    let data_dir = c.core.data_directory.to_owned();

    if data_dir.is_none() {
        return Err("data directory is missing in configuration file".into());
    }

    Ok(c.to_owned())
}
