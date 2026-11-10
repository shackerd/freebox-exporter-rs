#[cfg(test)]
mod test {
    use std::path::Path;

    use tokio::{
        fs::{self, File},
        io::AsyncWriteExt,
    };

    use crate::core::configuration::{
        get_configuration,
        sections::{
            ApiConfiguration, CapabilitiesConfiguration, CoreConfiguration, LogConfiguration,
            PoliciesConfiguration,
        },
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
        let content = "[api]
# interval in seconds
refresh = 5

[metrics]
connection = true
lan = true
lan_browser = true
switch = true
wifi = true
download = true
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
        assert_eq!(true, conf.metrics.download.unwrap());
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
                connect_timeout: None,
                request_timeout: None,
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
                nas: None,
                download: None,
                vm: None,
                raid: None,
                vpn: None,
                ddns: None,
            },
            policies: Some(PoliciesConfiguration {
                unresolved_station_hostnames: None,
            }),
        };

        let conf2 = Configuration {
            api: ApiConfiguration {
                refresh: None,
                connect_timeout: None,
                request_timeout: None,
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
                nas: None,
                download: None,
                vm: None,
                raid: None,
                vpn: None,
                ddns: None,
            },
            policies: Some(PoliciesConfiguration {
                unresolved_station_hostnames: None,
            }),
        };

        let conf3 = Configuration {
            api: ApiConfiguration {
                refresh: None,
                connect_timeout: None,
                request_timeout: None,
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
                nas: None,
                download: None,
                vm: None,
                raid: None,
                vpn: None,
                ddns: None,
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
                connect_timeout: None,
                request_timeout: None,
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
                nas: None,
                download: None,
                vm: None,
                raid: None,
                vpn: None,
                ddns: None,
            },
            policies: Some(PoliciesConfiguration {
                unresolved_station_hostnames: None,
            }),
        };

        let conf2 = Configuration {
            api: ApiConfiguration {
                refresh: None,
                connect_timeout: None,
                request_timeout: None,
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
                nas: None,
                download: None,
                vm: None,
                raid: None,
                vpn: None,
                ddns: None,
            },
            policies: Some(PoliciesConfiguration {
                unresolved_station_hostnames: None,
            }),
        };

        let conf3 = Configuration {
            api: ApiConfiguration {
                refresh: None,
                connect_timeout: None,
                request_timeout: None,
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
                prefix: Some("valid_prefix".to_string()),
                lan_browser: None,
                lan: None,
                switch: None,
                wifi: None,
                dhcp: None,
                nas: None,
                download: None,
                vm: None,
                raid: None,
                vpn: None,
                ddns: None,
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

#[cfg(test)]
mod timeouts {
    use std::time::Duration;

    use crate::core::{
        common::http_client_factory::Timeouts,
        configuration::{assert_configuration_is_valid, Configuration},
    };

    fn parse(api: &str) -> Configuration {
        toml::from_str(&format!(
            "[api]\n{api}\n[metrics]\nprefix = \"fbx\"\n[core]\ndata_directory = \".\"\n[log]\n"
        ))
        .unwrap()
    }

    #[test]
    fn timeouts_default_to_2_seconds_to_connect_and_5_seconds_per_request() {
        let expected = Timeouts {
            connect: Duration::from_secs(2),
            request: Duration::from_secs(5),
        };
        assert_eq!(expected, parse("refresh = 5").api.timeouts());
    }

    #[test]
    fn timeouts_follow_the_configuration() {
        let expected = Timeouts {
            connect: Duration::from_secs(3),
            request: Duration::from_secs(7),
        };
        assert_eq!(
            expected,
            parse("connect_timeout = 3\nrequest_timeout = 7")
                .api
                .timeouts()
        );
    }

    #[test]
    fn a_zero_timeout_is_rejected() {
        assert!(
            assert_configuration_is_valid(&parse("connect_timeout = 1\nrequest_timeout = 1"))
                .is_ok()
        );
        assert!(assert_configuration_is_valid(&parse("connect_timeout = 0")).is_err());
        assert!(assert_configuration_is_valid(&parse("request_timeout = 0")).is_err());
    }
}
