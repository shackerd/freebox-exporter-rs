#[cfg(test)]
mod tests_deserialize {
    use serde_json::from_str;

    use crate::{
        core::common::transport::FreeboxResponse,
        mappers::{
            api_specs_provider::get_specs_data,
            wifi::{
                models::{
                    ChannelSurveyHistory, ChannelUsage, NeighborsAccessPoint, Station, WifiConfig,
                },
                utils::calculate_avg_channel_survey_history,
            },
        },
    };

    #[tokio::test]
    async fn deserialize_api_v2_wifi_config() {
        let json_data = get_specs_data("wifi", "api_v2_wifi_config-get")
            .await
            .unwrap();

        let data: Result<FreeboxResponse<WifiConfig>, _> = from_str(&json_data);

        if let Ok(e) = &data {
            println!("{:?}", e);
        }

        assert!(data.is_ok());
    }

    #[tokio::test]
    async fn deserialize_api_v2_wifi_ap_0_stations() {
        let json_data = get_specs_data("wifi", "api_v2_wifi_ap_0_stations-get")
            .await
            .unwrap();

        let data: Result<FreeboxResponse<Vec<Station>>, _> = from_str(&json_data);

        if let Ok(e) = &data {
            println!("{:?}", e);
        }

        assert!(data.is_ok());
    }

    #[tokio::test]
    async fn deserialize_api_latest_wifi_ap_0_channel_survey_history() {
        let json_data = get_specs_data("wifi", "api_latest_wifi_ap_0_channel_survey_history-get")
            .await
            .unwrap();

        let data: Result<FreeboxResponse<Vec<ChannelSurveyHistory>>, _> = from_str(&json_data);

        assert!(data.is_ok());

        let avg_history =
            calculate_avg_channel_survey_history(data.unwrap().result.as_ref().unwrap());

        assert_eq!(avg_history.busy_percent.unwrap(), 34);
        assert_eq!(avg_history.tx_percent.unwrap(), 1);
        assert_eq!(avg_history.rx_percent.unwrap(), 30);
        assert_eq!(avg_history.rx_bss_percent.unwrap(), 0);
    }

    #[tokio::test]
    async fn deserialize_api_latest_ap_neighbors() {
        let json_data = get_specs_data("wifi", "api_latest_wifi_ap_0_neighbors-get")
            .await
            .unwrap();

        let data: Result<FreeboxResponse<Vec<NeighborsAccessPoint>>, _> = from_str(&json_data);

        if let Ok(e) = &data {
            println!("{:?}", e);
        }

        assert!(data.is_ok());
    }

    #[tokio::test]
    async fn deserialize_api_latest_ap_channel_usage() {
        let json_data = get_specs_data("wifi", "api_latest_wifi_ap_0_channel_usage-get")
            .await
            .unwrap();

        let data: Result<FreeboxResponse<Vec<ChannelUsage>>, _> = from_str(&json_data);

        if let Ok(e) = &data {
            println!("{:?}", e);
        }

        assert!(data.is_ok());
    }
}

#[cfg(test)]
mod tests_collect {
    use chrono::Duration;
    use prometheus_exporter::prometheus::core::Collector;
    use serde_json::json;
    use wiremock::{
        matchers::{path, path_regex},
        Mock, ResponseTemplate,
    };

    use crate::{
        core::configuration::sections::PoliciesConfiguration,
        mappers::{
            api_specs_provider::get_specs_data,
            test_support::{mock_api, scrape, scrape_during, serve, serve_scoped, serve_slowly},
            wifi::WifiMetricMap,
            MetricMap,
        },
    };

    #[tokio::test]
    async fn set_fails_when_no_expected_access_point_can_be_fetched() {
        let (server, authenticator) = mock_api().await;
        serve(
            &server,
            "/api/v4/wifi/ap",
            json!({ "success": true, "result": [] }),
        )
        .await;
        serve(
            &server,
            "/api/v4/wifi/config",
            json!({ "success": true, "result": { "enabled": true, "expected_phys": [{ "phy_id": 0 }] } }),
        )
        .await;
        serve(
            &server,
            "/api/v4/wifi/ap/0",
            json!({ "success": false, "error_code": "internal_error", "msg": "boom" }),
        )
        .await;

        let factory = authenticator.login().await.unwrap();
        let policies = PoliciesConfiguration {
            unresolved_station_hostnames: Some("ignore".to_string()),
        };
        let mut map = WifiMetricMap::new(
            &factory,
            "test_wifi_no_ap".to_string(),
            Duration::seconds(5),
            &policies,
        );

        let err = map.set().await.unwrap_err();
        assert!(
            err.to_string()
                .contains("none of the 1 expected access points"),
            "{err}"
        );
    }

    #[tokio::test]
    async fn set_collects_reachable_access_points_and_reports_the_failed_ones() {
        let (server, authenticator) = mock_api().await;
        serve(
            &server,
            "/api/v4/wifi/ap",
            json!({ "success": true, "result": [] }),
        )
        .await;
        serve(
            &server,
            "/api/v4/wifi/config",
            json!({ "success": true, "result": { "enabled": true, "expected_phys": [{ "phy_id": 0 }, { "phy_id": 1 }] } }),
        )
        .await;
        serve(
            &server,
            "/api/v4/wifi/ap/0",
            json!({ "success": false, "error_code": "internal_error", "msg": "boom" }),
        )
        .await;
        serve(
            &server,
            "/api/v4/wifi/ap/1",
            json!({ "success": true, "result": { "id": 1, "name": "5G", "config": { "band": "5g" } } }),
        )
        .await;
        Mock::given(path_regex(
            r"^/api/v4/wifi/ap/1/channel_survey_history/\d+$",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "success": true })))
        .mount(&server)
        .await;
        let stations = get_specs_data("wifi", "api_v2_wifi_ap_0_stations-get")
            .await
            .unwrap();
        Mock::given(path("/api/v4/wifi/ap/1/stations"))
            .respond_with(ResponseTemplate::new(200).set_body_raw(stations, "application/json"))
            .expect(1)
            .mount(&server)
            .await;

        let factory = authenticator.login().await.unwrap();
        let policies = PoliciesConfiguration {
            unresolved_station_hostnames: Some("relabel".to_string()),
        };
        let mut map = WifiMetricMap::new(
            &factory,
            "test_wifi_partial_ap".to_string(),
            Duration::seconds(5),
            &policies,
        );

        let err = map.set().await.unwrap_err();
        assert!(err.to_string().contains("phy_id 0"), "{err}");
        assert!(!err.to_string().contains("phy_id 0, 1"), "{err}");

        let stations_of_ap_1 = map
            .station_active_gauge
            .collect()
            .iter()
            .flat_map(|family| family.get_metric())
            .filter(|m| {
                m.get_label()
                    .iter()
                    .any(|l| l.get_name() == "ap_id" && l.get_value() == "1")
            })
            .count();
        assert!(
            stations_of_ap_1 > 0,
            "stations of the reachable access point are not exposed"
        );
    }

    #[tokio::test]
    async fn scrape_during_collection_sees_the_previous_collection() {
        let (server, authenticator) = mock_api().await;
        let ap = json!({ "success": true, "result": [{ "id": 0, "name": "2G4", "config": { "band": "2d4g" } }] });
        serve(&server, "/api/v4/wifi/ap", ap).await;
        Mock::given(path_regex(
            r"^/api/v4/wifi/ap/0/channel_survey_history/\d+$",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "success": true })))
        .mount(&server)
        .await;
        let unavailable = json!({ "success": false, "error_code": "invalid_request" });
        serve(
            &server,
            "/api/v4/wifi/ap/0/channel_usage",
            unavailable.clone(),
        )
        .await;
        serve(&server, "/api/v4/wifi/ap/0/neighbors", unavailable).await;

        let factory = authenticator.login().await.unwrap();
        let policies = PoliciesConfiguration {
            unresolved_station_hostnames: Some("relabel".to_string()),
        };
        let prefix = "test_wifi_scrape";
        let mut map = WifiMetricMap::new(
            &factory,
            prefix.to_string(),
            Duration::seconds(5),
            &policies,
        );

        let stations = get_specs_data("wifi", "api_v2_wifi_ap_0_stations-get")
            .await
            .unwrap();
        let stations: serde_json::Value = serde_json::from_str(&stations).unwrap();
        let first = serve_scoped(&server, "/api/v4/wifi/ap/0/stations", stations).await;
        map.set().await.unwrap();
        let before = scrape(prefix);
        assert!(
            before
                .iter()
                .any(|l| l.starts_with(&format!("{prefix}_wifi_station_active"))),
            "{before:?}"
        );
        drop(first);

        // the last request of the collection is slow, and its stations are gone
        serve_slowly(
            &server,
            "/api/v4/wifi/ap/0/stations",
            json!({ "success": true, "result": [] }),
        )
        .await;
        let (result, during) =
            scrape_during(&server, "/api/v4/wifi/ap/0/stations", map.set(), prefix).await;
        result.unwrap();

        assert_eq!(before, during);
        assert!(!scrape(prefix)
            .iter()
            .any(|l| l.starts_with(&format!("{prefix}_wifi_station_active"))));
    }
}
