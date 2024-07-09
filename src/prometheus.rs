use std::net::SocketAddr;

use prometheus_exporter::prometheus::register_gauge;

use crate::common::FreeboxClient;

pub struct Server {
    port: u16,
    client: FreeboxClient
}

impl Server {
    pub fn new(port: u16, client: FreeboxClient) -> Self {
        Self {
            port,
            client
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
            let connection = self.client.test().await.unwrap();

            metric1.set(connection.result.bytes_down as f64);

            i = i + 1;
        }


    }
}

pub struct Translator {
    client: FreeboxClient
}
