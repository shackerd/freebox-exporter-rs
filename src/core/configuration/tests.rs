#[cfg(test)]
mod test {
    use std::path::Path;

    use tokio::{
        fs::{self, File},
        io::AsyncWriteExt,
    };

    use crate::core::configuration::{
        get_configuration,
        sections::{ApiConfiguration, CoreConfiguration, LogConfiguration, CapabilitiesConfiguration, PoliciesConfiguration},
        Configuration,
    };

    async fn create_sample_file(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
        if path.exists() {
            fs::remove_file(path)
                .await
                .expect("cannot remove sample configuration file");
        }

        let mut file = File::create(path)
            .await
            .expect("cannot create sample configuration file");
        let content =
"[api]
# interval in seconds
refresh = 5

[metrics]
connection = true
lan = true
lan_browser = true
switch = true
wifi = true
system = false
prefix = \"fbx\"

[core]
data_directory = \".\"
port = 9102

[log]
level = \"Info\"
retention = 31

[policies]
unresolved_station_hostnames = \"ignore\"";

        file.write_all(content.as_bytes())
            .await
            .expect("cannot write to sample configuration file");
        file.shutdown().await?;

        Ok(())
    }

    #[tokio::test]
    async fn should_match_expected_values() {
        let path = Path::new("./test_conf.toml");

        create_sample_file(path).await.unwrap();

        let conf = get_configuration("./test_conf.toml".to_string())
            .await
            .expect("cannot load configuration");

        fs::remove_file(path)
            .await
            .expect("cannot cleanup sample configuration file");

        assert_eq!(5, conf.api.refresh.unwrap());

        assert_eq!(true, conf.metrics.connection.unwrap());
        assert_eq!(true, conf.metrics.lan.unwrap());
        assert_eq!(true, conf.metrics.lan_browser.unwrap());
        assert_eq!(true, conf.metrics.switch.unwrap());
        assert_eq!(true, conf.metrics.wifi.unwrap());
        assert_eq!(false, conf.metrics.system.unwrap());
        assert_eq!("fbx", conf.metrics.prefix.unwrap());

        assert_eq!(".".to_string(), conf.core.data_directory.unwrap());
        assert_eq!(9102, conf.core.port.unwrap());
        assert_eq!("Info", conf.log.level.unwrap());
        assert_eq!(31, conf.log.retention.unwrap());
    }

    #[test]
    fn assert_data_dir_permissions_tests() {
        let conf = Configuration {
            api: ApiConfiguration {
                refresh: None,
            },
            core: CoreConfiguration {
                data_directory: Some("nowhere".to_string()),
                port: None,
            },
            log: LogConfiguration {
                level: None,
                retention: None,
            },
            metrics: CapabilitiesConfiguration {
                connection: None,
                system: None,
                prefix: None,
                lan_browser: None,
                lan: None,
                switch: None,
                wifi: None,
                dhcp: None,
            },
            policies: Some(PoliciesConfiguration {
                unresolved_station_hostnames: None,
            }),
        };

        let conf2 = Configuration {
            api: ApiConfiguration {                
                refresh: None,
            },
            core: CoreConfiguration {
                data_directory: Some("".to_string()),
                port: None,
            },
            log: LogConfiguration {
                level: None,
                retention: None,
            },
            metrics: CapabilitiesConfiguration {
                connection: None,
                system: None,
                prefix: None,
                lan_browser: None,
                lan: None,
                switch: None,
                wifi: None,
                dhcp: None,
            },
            policies: Some(PoliciesConfiguration {
                unresolved_station_hostnames: None,
            }),
        };

        let conf3 = Configuration {
            api: ApiConfiguration {
                refresh: None,
            },
            core: CoreConfiguration {
                data_directory: Some(".".to_string()),
                port: None,
            },
            log: LogConfiguration {
                level: None,
                retention: None,
            },
            metrics: CapabilitiesConfiguration {
                connection: None,
                system: None,
                prefix: None,
                lan_browser: None,
                lan: None,
                switch: None,
                wifi: None,
                dhcp: None,
            },
            policies: Some(PoliciesConfiguration {
                unresolved_station_hostnames: None,
            }),
        };

        assert_eq!(true, conf.assert_data_dir_permissions().is_err());
        assert_eq!(true, conf2.assert_data_dir_permissions().is_err());
        assert_eq!(true, conf3.assert_data_dir_permissions().is_ok());
    }

    #[test]
    fn assert_metrics_prefix_is_not_empty_tests() {
        let conf = Configuration {
            api: ApiConfiguration {
                refresh: None,
            },
            core: CoreConfiguration {
                data_directory: None,
                port: None,
            },
            log: LogConfiguration {
                level: None,
                retention: None,
            },
            metrics: CapabilitiesConfiguration {
                connection: None,
                system: None,
                prefix: None,
                lan_browser: None,
                lan: None,
                switch: None,
                wifi: None,
                dhcp: None,
            },
            policies: Some(PoliciesConfiguration {
                unresolved_station_hostnames: None,
            }),
        };

        let conf2 = Configuration {
            api: ApiConfiguration {
                refresh: None,
            },
            core: CoreConfiguration {
                data_directory: None,
                port: None,
            },
            log: LogConfiguration {
                level: None,
                retention: None,
            },
            metrics: CapabilitiesConfiguration {
                connection: None,
                system: None,
                prefix: Some(" ".to_string()),
                lan_browser: None,
                lan: None,
                switch: None,
                wifi: None,
                dhcp: None,
            },
            policies: Some(PoliciesConfiguration {
                unresolved_station_hostnames: None,
            }),
        };

        let conf3 = Configuration {
            api: ApiConfiguration {
                refresh: None,
            },
            core: CoreConfiguration {
                data_directory: None,
                port: None,
            },
            log: LogConfiguration {
                level: None,
                retention: None,
            },
            metrics: CapabilitiesConfiguration {
                connection: None,
                system: None,
                prefix: Some("fbx_exporter".to_string()),
                lan_browser: None,
                lan: None,
                switch: None,
                wifi: None,
                dhcp: None,
            },
            policies: Some(PoliciesConfiguration {
                unresolved_station_hostnames: None,
            }),
        };

        assert_eq!(Err(()), conf.assert_metrics_prefix_is_not_empty());
        assert_eq!(Err(()), conf2.assert_metrics_prefix_is_not_empty());
        assert_eq!(Ok(()), conf3.assert_metrics_prefix_is_not_empty());
    }
}
