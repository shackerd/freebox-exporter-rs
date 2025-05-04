#[cfg(test)]
mod tests_deserialize {
    use serde_json::from_str;

    use crate::{core::common::transport::FreeboxResponse, mappers::{api_specs_provider::get_specs_data, wifi::{models::{ChannelSurveyHistory, ChannelUsage, NeighborsAccessPoint, Station, WifiConfig}, utils::calculate_avg_channel_survey_history}}};

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
