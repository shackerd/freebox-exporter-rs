use serde::Deserialize;
use tokio::{fs::File, io::AsyncReadExt};
use std::{fs::{self}, path::Path};

#[derive(Deserialize, Clone, Debug)]
pub struct Configuration {
    pub api: ApiConfiguration,
    pub metrics: MetricsConfiguration,
    pub core: CoreConfiguration,
    pub log: LogConfiguration
}

#[derive(Deserialize, Clone, Debug)]
pub struct CoreConfiguration {
    pub data_directory: Option<String>,
    pub port: Option<u16>
}

#[derive(Deserialize, Clone, Debug)]
pub struct ApiConfiguration {
    pub mode : Option<String>,
    pub refresh: Option<u64>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct MetricsConfiguration {
    pub connection: Option<bool>,
    pub system: Option<bool>,
    pub lan: Option<bool>,
    pub lan_browser: Option<bool>,
    pub switch: Option<bool>,
    pub contacts: Option<bool>,
    pub calls: Option<bool>,
    pub explorer: Option<bool>,
    pub downloader: Option<bool>,
    pub parental: Option<bool>,
    pub pvr: Option<bool>,
    pub prefix: Option<String>
}

#[derive(Deserialize, Clone, Debug)]
pub struct LogConfiguration {
    pub level: Option<String>,
    pub retention: Option<usize>
}

impl Configuration {
    pub fn assert_data_dir_permissions(&self) -> Result<(), &str> {

        let data_dir = self.core.data_directory.to_owned().unwrap();

        let path = Path::new(&data_dir);

        if !path.try_exists().expect("Access is denied") {
            return Err("data dir does not exist");
        }

        let permissions =
            fs::metadata(path).expect("cannot read metadata").permissions();

        if permissions.readonly() {
            return Err("data_dir cannot be readonly");
        }

        Ok(())
    }

    pub fn assert_metrics_prefix_is_not_empty(&self) -> Result<(), ()> {

        self.metrics.prefix.clone()
            .map_or_else(
                || Err(()),
                |v| match v.trim() { "" => { Err(()) }, _ => { Ok(()) } })
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
        Err(e) => {
            println!("{e:#?}");
            panic!("Configuration file is corrupted");
        }
    }
}

#[cfg(test)]
mod test {
    use std::path::Path;

    use tokio::{fs::{self, File}, io::AsyncWriteExt};

    use crate::core::configuration::get_configuration;

    use super::{ApiConfiguration, Configuration, CoreConfiguration, LogConfiguration, MetricsConfiguration};

    async fn create_sample_file(path: &Path) -> Result<(), Box<dyn std::error::Error>>{

        if path.exists() {
            fs::remove_file(path).await.expect("cannot remove sample configuration file");
        }

        let mut file = File::create(path).await.expect("cannot create sample configuration file");
        let content =
"[api]
# acceptable values: \"router\" or \"bridge\"
# this option will determine whether use discovery or not see: https://github.com/shackerd/freebox-exporter-rs/issues/2#issuecomment-2234856496
mode = \"bridge\"

# interval in seconds
refresh = 5

[metrics]
connection = true
lan = true
lan_browser = true
switch = true
system = false
contacts = true
calls = true
explorer = true
downloader = true
parental = true
pvr = true
prefix = \"fbx\"

[core]
data_directory = \".\"
port = 9102
[log]
level = \"Info\"
retention = 31";

        file.write_all(content.as_bytes()).await.expect("cannot write to sample configuration file");
        file.shutdown().await?;

        Ok(())
    }

    #[tokio::test]
    async fn should_match_expected_values() {

        let path = Path::new("./test_conf.toml");

        create_sample_file(path).await.unwrap();

        let conf = get_configuration("./test_conf.toml".to_string()).await.expect("cannot load configuration");

        fs::remove_file(path).await.expect("cannot cleanup sample configuration file");

        assert_eq!("bridge", conf.api.mode.unwrap());
        assert_eq!(5, conf.api.refresh.unwrap());

        assert_eq!(true, conf.metrics.connection.unwrap());
        assert_eq!(true, conf.metrics.lan.unwrap());
        assert_eq!(true, conf.metrics.lan_browser.unwrap());
        assert_eq!(true, conf.metrics.switch.unwrap());
        assert_eq!(false, conf.metrics.system.unwrap());
        assert_eq!(true, conf.metrics.contacts.unwrap());
        assert_eq!(true, conf.metrics.calls.unwrap());
        assert_eq!(true, conf.metrics.explorer.unwrap());
        assert_eq!(true, conf.metrics.downloader.unwrap());
        assert_eq!(true, conf.metrics.parental.unwrap());
        assert_eq!(true, conf.metrics.pvr.unwrap());
        assert_eq!("fbx", conf.metrics.prefix.unwrap());

        assert_eq!(".".to_string(), conf.core.data_directory.unwrap());
        assert_eq!(9102, conf.core.port.unwrap());
        assert_eq!("Info", conf.log.level.unwrap());
        assert_eq!(31, conf.log.retention.unwrap());
    }

    #[test]
    fn assert_data_dir_permissions_tests() {
        let conf = Configuration {
            api: ApiConfiguration { mode: None, refresh: None},
            core: CoreConfiguration { data_directory: Some("nowhere".to_string()), port: None },
            log: LogConfiguration { level: None, retention: None },
            metrics: MetricsConfiguration {
                calls: None, connection: None, contacts: None,
                downloader: None, explorer: None, parental: None,
                pvr: None, system: None, prefix: None, lan_browser: None,
                lan: None, switch: None
            }
        };

        let conf2 = Configuration {
            api: ApiConfiguration { mode: None, refresh: None},
            core: CoreConfiguration { data_directory: Some("".to_string()), port: None },
            log: LogConfiguration { level: None, retention: None },
            metrics: MetricsConfiguration {
                calls: None, connection: None, contacts: None,
                downloader: None, explorer: None, parental: None,
                pvr: None, system: None, prefix: None,lan_browser: None,
                lan: None, switch: None
            }
        };

        let conf3 = Configuration {
            api: ApiConfiguration { mode: None, refresh: None},
            core: CoreConfiguration { data_directory: Some(".".to_string()), port: None },
            log: LogConfiguration { level: None, retention: None },
            metrics: MetricsConfiguration {
                calls: None, connection: None, contacts: None,
                downloader: None, explorer: None, parental: None,
                pvr: None, system: None, prefix: None, lan_browser: None,
                lan: None, switch: None
            }
        };

        assert_eq!(true, conf.assert_data_dir_permissions().is_err());
        assert_eq!(true, conf2.assert_data_dir_permissions().is_err());
        assert_eq!(true, conf3.assert_data_dir_permissions().is_ok());
    }

    #[test]
    fn assert_metrics_prefix_is_not_empty_tests() {

        let conf = Configuration {
            api: ApiConfiguration { mode: None, refresh: None},
            core: CoreConfiguration { data_directory: None, port: None },
            log: LogConfiguration { level: None, retention: None },
            metrics: MetricsConfiguration {
                calls: None, connection: None, contacts: None,
                downloader: None, explorer: None, parental: None,
                pvr: None, system: None, prefix: None, lan_browser: None,
                lan: None, switch: None
            }
        };

        let conf2 = Configuration {
            api: ApiConfiguration { mode: None, refresh: None},
            core: CoreConfiguration { data_directory: None, port: None },
            log: LogConfiguration { level: None, retention: None },
            metrics: MetricsConfiguration {
                calls: None, connection: None, contacts: None,
                downloader: None, explorer: None, parental: None,
                pvr: None, system: None, prefix: Some(" ".to_string()),
                lan_browser: None, lan: None, switch: None
            }
        };

        let conf3 = Configuration {
            api: ApiConfiguration { mode: None, refresh: None},
            core: CoreConfiguration { data_directory: None, port: None },
            log: LogConfiguration { level: None, retention: None },
            metrics: MetricsConfiguration {
                calls: None, connection: None, contacts: None,
                downloader: None, explorer: None, parental: None,
                pvr: None, system: None, prefix: Some("fbx_exporter".to_string()),
                lan_browser: None, lan: None, switch: None
            }
        };

        assert_eq!(Err(()), conf.assert_metrics_prefix_is_not_empty());
        assert_eq!(Err(()), conf2.assert_metrics_prefix_is_not_empty());
        assert_eq!(Ok(()), conf3.assert_metrics_prefix_is_not_empty());

    }
}
