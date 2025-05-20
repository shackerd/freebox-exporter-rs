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