use serde::Deserialize;
use tokio::{fs::File, io::AsyncReadExt};
use std::{fs::{self}, path::Path};

use crate::core::common::Permissions;

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
    pub expose: Permissions,
    pub refresh_interval_secs: u64
}

impl Configuration {
    pub fn assert_data_dir_permissions(&self) -> Result<(), Box<dyn std::error::Error>> {
        let path = Path::new(&self.core.data_dir);

        if !path.try_exists().expect("Access is denied") {
            panic!("data dir does not exist");
        }

        let permissions =
            fs::metadata(path).expect("cannot read metadata").permissions();

        if permissions.readonly() {
            panic!("data_dir cannot be readonly");
        }

        Ok(())
    }
}

pub async fn get_configuration(file_path: String) -> Result<Configuration, Box<dyn std::error::Error>> {
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
            return Ok(c);
        },
        Err(_) => {
            panic!("Configuration file is corrupted");
        }
    }
}

#[cfg(test)]
mod test {
    use std::path::Path;

    use tokio::{fs::{self, File}, io::AsyncWriteExt};

    use crate::core::configuration::get_configuration;

    async fn create_sample_file(path: &Path) -> Result<(), Box<dyn std::error::Error>>{

        if path.exists() {
            fs::remove_file(path).await.expect("cannot remove sample configuration file");
        }

        let mut file = File::create(path).await.expect("cannot create sample configuration file");
        let content =
"[api]
host = \"mafreebox.freebox.fr\"
port = 443
expose = { connection = true,  settings = false, contacts = true, calls = true, explorer = true, downloader = true, parental = true, pvr = true }

[core]
data_dir = \"/usr/share/freebox-exporter\"
port = 9102";

        file.write_all(content.as_bytes()).await.expect("cannot write to sample configuration file");
        file.shutdown().await?;

        Ok(())
    }

    #[tokio::test]
    async fn should_match_expected_values() {

        let path = Path::new("./src/test_conf.toml");

        create_sample_file(path).await.unwrap();

        let conf = get_configuration("./src/test_conf.toml".to_string()).await.expect("cannot load configuration");

        fs::remove_file(path).await.expect("cannot cleanup sample configuration file");

        assert_eq!(conf.api.host, "mafreebox.freebox.fr".to_string());
        assert_eq!(conf.api.port, 443);
        assert_eq!(conf.api.expose.connection, true);
        assert_eq!(conf.api.expose.settings, false);
        assert_eq!(conf.api.expose.contacts, true);
        assert_eq!(conf.api.expose.calls, true);
        assert_eq!(conf.api.expose.explorer, true);
        assert_eq!(conf.api.expose.downloader, true);
        assert_eq!(conf.api.expose.parental, true);
        assert_eq!(conf.api.expose.pvr, true);

        assert_eq!(conf.core.data_dir, "/usr/share/freebox-exporter".to_string());
        assert_eq!(conf.core.port, 9102);
    }
}
