#[cfg(test)]
mod tests {
    use crate::{
        core::common::transport::FreeboxResponse,
        mappers::{api_specs_provider::get_specs_data, connection::models::XdslInfo},
    };
    use serde_json::from_str;

    #[tokio::test]
    async fn deserialize_api_v4_connection_xdsl() {
        let json_data = get_specs_data("connection", "api_v4_connection_xdsl-get")
            .await
            .unwrap();

        let data: Result<FreeboxResponse<XdslInfo>, _> = from_str(&json_data);

        if let Ok(e) = &data {
            println!("{:?}", e);
        }

        assert!(data.is_ok());
    }
}
#[cfg(test)]
mod tests_collect {
    use serde_json::{json, Value};

    use crate::mappers::{
        connection::ConnectionMetricMap,
        test_support::{
            collect_while_locked, mock_api, new_update_lock, scrape, scrape_during, serve,
            serve_scoped, serve_slowly, update_lock,
        },
        MetricMap,
    };

    fn ftth(sfp_pwr_rx: i64) -> Value {
        json!({ "success": true, "result": { "link": true, "sfp_pwr_rx": sfp_pwr_rx } })
    }

    #[tokio::test]
    async fn scrape_during_collection_sees_the_previous_collection() {
        let (server, authenticator) = mock_api().await;
        serve(
            &server,
            "/api/v4/connection",
            json!({ "success": true, "result": {
            "type": "ethernet", "state": "up", "media": "ftth", "bytes_down": 10
        } }),
        )
        .await;
        serve(
            &server,
            "/api/v4/connection/config",
            json!({ "success": true, "result": { "ping": true } }),
        )
        .await;
        serve(
            &server,
            "/api/v4/connection/ipv6/config",
            json!({ "success": true, "result": { "ipv6_enabled": true } }),
        )
        .await;

        let factory = authenticator.login().await.unwrap();
        let prefix = "test_connection_scrape";
        let mut map = ConnectionMetricMap::new(&factory, prefix.to_string());

        let first = serve_scoped(&server, "/api/v4/connection/ftth", ftth(-1500)).await;
        map.set(update_lock()).await.unwrap();
        let before = scrape(prefix);
        assert!(
            before.contains(&format!("{prefix}_connection_ftth_sfp_pwr_rx -1500")),
            "{before:?}"
        );
        assert!(
            before.contains(&format!("{prefix}_connection_bytes_down 10")),
            "{before:?}"
        );
        drop(first);

        // the last request of the collection is slow, and returns a new power level
        serve_slowly(&server, "/api/v4/connection/ftth", ftth(-1600)).await;
        let (result, during) = scrape_during(
            &server,
            "/api/v4/connection/ftth",
            map.set(update_lock()),
            prefix,
        )
        .await;
        result.unwrap();

        assert_eq!(before, during);
        assert!(scrape(prefix).contains(&format!("{prefix}_connection_ftth_sfp_pwr_rx -1600")));
    }

    #[tokio::test]
    async fn collection_waits_for_a_scrape_in_progress() {
        let (server, authenticator) = mock_api().await;
        serve(
            &server,
            "/api/v4/connection",
            json!({ "success": true, "result": {
            "type": "ethernet", "state": "up", "media": "ftth", "bytes_down": 10
        } }),
        )
        .await;
        serve(
            &server,
            "/api/v4/connection/config",
            json!({ "success": true, "result": { "ping": true } }),
        )
        .await;
        serve(
            &server,
            "/api/v4/connection/ipv6/config",
            json!({ "success": true, "result": { "ipv6_enabled": true } }),
        )
        .await;

        let factory = authenticator.login().await.unwrap();
        let prefix = "test_connection_lock";
        let mut map = ConnectionMetricMap::new(&factory, prefix.to_string());

        let first = serve_scoped(&server, "/api/v4/connection/ftth", ftth(-1500)).await;
        map.set(update_lock()).await.unwrap();
        let before = scrape(prefix);
        assert!(
            before.contains(&format!("{prefix}_connection_ftth_sfp_pwr_rx -1500")),
            "{before:?}"
        );
        assert!(
            before.contains(&format!("{prefix}_connection_bytes_down 10")),
            "{before:?}"
        );
        drop(first);

        // a scrape holds the lock while the collection gets a new response
        serve(&server, "/api/v4/connection/ftth", ftth(-1600)).await;
        let lock = new_update_lock();
        let (result, during) = collect_while_locked(&lock, map.set(&lock), prefix).await;
        result.unwrap();

        assert_eq!(before, during);
        assert!(scrape(prefix).contains(&format!("{prefix}_connection_ftth_sfp_pwr_rx -1600")));
    }
}

#[cfg(test)]
mod tests_incomplete {
    use serde_json::{json, Value};

    use crate::mappers::{
        connection::ConnectionMetricMap,
        test_support::{mock_api, scrape, serve, serve_scoped, update_lock},
        MetricMap,
    };

    fn xdsl_status() -> Value {
        json!({ "success": true, "result": { "type": "ethernet", "state": "up", "media": "xdsl" } })
    }

    fn xdsl() -> Value {
        let stats =
            json!({ "maxrate": 30636, "rate": 28031, "snr": 7, "attn": 0, "fec": 0, "crc": 0 });
        json!({ "success": true, "result": {
            "status": { "status": "showtime", "protocol": "adsl2plus_a", "uptime": 5017, "modulation": "adsl" },
            "down": stats, "up": stats
        } })
    }

    fn ipv6_conf(delegation: Value) -> Value {
        json!({ "success": true, "result": { "ipv6_enabled": true, "delegations": [delegation] } })
    }

    #[tokio::test]
    async fn an_incomplete_xdsl_response_keeps_the_previous_collection() {
        for field in ["status", "up", "down"] {
            let (server, authenticator) = mock_api().await;
            serve(&server, "/api/v4/connection", xdsl_status()).await;
            serve(
                &server,
                "/api/v4/connection/config",
                json!({ "success": true, "result": { "ping": true } }),
            )
            .await;
            serve(
                &server,
                "/api/v4/connection/ipv6/config",
                json!({ "success": true, "result": { "ipv6_enabled": true } }),
            )
            .await;

            let factory = authenticator.login().await.unwrap();
            let prefix = format!("test_connection_incomplete_xdsl_{field}");
            let mut map = ConnectionMetricMap::new(&factory, prefix.clone());

            let complete = serve_scoped(&server, "/api/v4/connection/xdsl", xdsl()).await;
            map.set(update_lock()).await.unwrap();
            let before = scrape(&prefix);
            assert!(
                before.contains(&format!(
                    "{prefix}_connection_xdsl_stats_rate{{direction=\"up\"}} 28031"
                )),
                "{before:?}"
            );
            drop(complete);

            let mut incomplete = xdsl();
            incomplete["result"].as_object_mut().unwrap().remove(field);
            serve(&server, "/api/v4/connection/xdsl", incomplete).await;

            assert!(
                map.set(update_lock()).await.is_err(),
                "missing {field} must fail the collection"
            );
            assert_eq!(
                before,
                scrape(&prefix),
                "missing {field} must keep the previous collection"
            );
        }
    }

    #[tokio::test]
    async fn an_incomplete_ipv6_delegation_keeps_the_previous_collection() {
        for field in ["prefix", "next_hop"] {
            let (server, authenticator) = mock_api().await;
            serve(&server, "/api/v4/connection", xdsl_status()).await;
            serve(
                &server,
                "/api/v4/connection/config",
                json!({ "success": true, "result": { "ping": true } }),
            )
            .await;
            serve(&server, "/api/v4/connection/xdsl", xdsl()).await;

            let factory = authenticator.login().await.unwrap();
            let prefix = format!("test_connection_incomplete_ipv6_{field}");
            let mut map = ConnectionMetricMap::new(&factory, prefix.clone());

            let delegation = json!({ "prefix": "2001:db8::/64", "next_hop": "fe80::1" });
            let complete = serve_scoped(
                &server,
                "/api/v4/connection/ipv6/config",
                ipv6_conf(delegation.clone()),
            )
            .await;
            map.set(update_lock()).await.unwrap();
            let before = scrape(&prefix);
            assert!(before.iter().any(|l| l.starts_with(&format!("{prefix}_connection_ipv6_conf_delegations{{"))), "{before:?}");
            drop(complete);

            let mut incomplete = delegation;
            incomplete.as_object_mut().unwrap().remove(field);
            serve(
                &server,
                "/api/v4/connection/ipv6/config",
                ipv6_conf(incomplete),
            )
            .await;

            assert!(
                map.set(update_lock()).await.is_err(),
                "missing {field} must fail the collection"
            );
            assert_eq!(
                before,
                scrape(&prefix),
                "missing {field} must keep the previous collection"
            );
        }
    }
}
