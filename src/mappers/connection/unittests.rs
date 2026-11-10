#[cfg(test)]
mod tests {
    use crate::{core::common::transport::FreeboxResponse, mappers::{api_specs_provider::get_specs_data, connection::models::XdslInfo}};
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
        test_support::{mock_api, scrape, scrape_during, serve, serve_scoped, serve_slowly},
        MetricMap,
    };

    fn ftth(sfp_pwr_rx: i64) -> Value {
        json!({ "success": true, "result": { "link": true, "sfp_pwr_rx": sfp_pwr_rx } })
    }

    #[tokio::test]
    async fn scrape_during_collection_sees_the_previous_collection() {
        let (server, authenticator) = mock_api().await;
        serve(&server, "/api/v4/connection", json!({ "success": true, "result": {
            "type": "ethernet", "state": "up", "media": "ftth", "bytes_down": 10
        } })).await;
        serve(&server, "/api/v4/connection/config", json!({ "success": true, "result": { "ping": true } })).await;
        serve(&server, "/api/v4/connection/ipv6/config", json!({ "success": true, "result": { "ipv6_enabled": true } })).await;

        let factory = authenticator.login().await.unwrap();
        let prefix = "test_connection_scrape";
        let mut map = ConnectionMetricMap::new(&factory, prefix.to_string());

        let first = serve_scoped(&server, "/api/v4/connection/ftth", ftth(-1500)).await;
        map.set().await.unwrap();
        let before = scrape(prefix);
        assert!(before.contains(&format!("{prefix}_connection_ftth_sfp_pwr_rx -1500")), "{before:?}");
        assert!(before.contains(&format!("{prefix}_connection_bytes_down 10")), "{before:?}");
        drop(first);

        // the last request of the collection is slow, and returns a new power level
        serve_slowly(&server, "/api/v4/connection/ftth", ftth(-1600)).await;
        let (result, during) = scrape_during(map.set(), prefix).await;
        result.unwrap();

        assert_eq!(before, during);
        assert!(scrape(prefix).contains(&format!("{prefix}_connection_ftth_sfp_pwr_rx -1600")));
    }
}
