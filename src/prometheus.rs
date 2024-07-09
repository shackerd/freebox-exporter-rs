use std::net::SocketAddr;

use prometheus_exporter::prometheus::register_gauge;
use serde::Deserialize;

use crate::common::{AuthenticatedHttpClientFactory, FreeboxResponse};

pub struct Server {
    port: u16,
    translator: Translator
}

impl Server {
    pub fn new(port: u16, translator: Translator) -> Self {
        Self {
            port,
            translator
        }
    }

    pub async fn run(&self) -> Result<(), Box<dyn std::error::Error>> {

        let addr_raw = format!("0.0.0.0:{}", self.port);
        let addr: SocketAddr = addr_raw.parse().expect("Cannot parse addr");
        let exporter = prometheus_exporter::start(addr).expect("Cannot start exporter");
        let duration = std::time::Duration::from_millis(5000);
        let metric1 = register_gauge!("bytes_down", "bytes_down").expect("cannot create test gauge");

        let mut i = 0;

        loop {
            let guard = exporter.wait_duration(duration);
            let connection = self.translator.connection().await.unwrap();

            metric1.set(connection.result.bytes_down as f64);

            i = i + 1;
        }


    }
}

pub struct Translator {
    factory: AuthenticatedHttpClientFactory
}

impl Translator {
    pub fn new(factory: AuthenticatedHttpClientFactory) -> Self {
        Self {
            factory
        }
    }

    pub async fn connection(&self) -> Result<FreeboxResponse<Connection>,()> {

        let body =
            self.factory.create_client().unwrap().get(format!("{}v4/connection", self.factory.api_url))
            .send().await.unwrap()
            .text().await.unwrap();

        let res = serde_json::from_str::<FreeboxResponse<Connection>>(&body);

        match res {
            Ok(c) => {
                return Ok(c);
            },
            Err(e) => {
                panic!("{}", e);
            }
        }
    }
}


#[derive(Deserialize, Debug)]
pub struct Connection {
    #[serde(alias="type")]
    pub _type: String,
    pub rate_down: u64,
    pub bytes_up: u64,
    pub rate_up: u64,
    pub bandwidth_up: u64,
    pub ipv4: String,
    pub ipv6: String,
    pub bandwidth_down: u64,
    pub state: String,
    pub bytes_down: u64,
    pub media: String
}