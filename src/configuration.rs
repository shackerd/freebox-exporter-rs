use serde::Deserialize;
use tokio::{fs::File, io::AsyncReadExt};
use std::path::Path;

use crate::common::Permissions;

#[derive(Deserialize, Debug)]
pub struct Configuration {
    pub api: ApiConfiguration,
    pub core: CoreConfiguration
}

#[derive(Deserialize, Debug)]
pub struct CoreConfiguration {
    pub data_dir: String,
    pub port: u16
}

#[derive(Deserialize, Debug)]
pub struct ApiConfiguration {
    pub host: String,
    pub port: u16,
    pub expose: Permissions
}

pub async fn get_configuration(file_path: String) -> Result<Configuration, Box<dyn std::error::Error>> {
    let path = Path::new(file_path.as_str());

    if !path.exists() {
        panic!("Configuration file is missing");
    }

    let mut file = File::open(path).await?;
    let mut buffer = vec![];

    file.read_to_end(&mut buffer).await?;

    let result = String::from_utf8(buffer)?;

    match toml::from_str::<Configuration>(&result) {
        Ok(c) => {
            return Ok(c);
        },
        Err(_) => {
            panic!("Configuration file is corrupted");
        }
    }
}
