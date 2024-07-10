use std::net::SocketAddr;

use crate::translators::Translator;

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

        let mut i = 0;

        loop {
            let _guard = exporter.wait_duration(duration);

            self.translator.set_all().await?;

            i = i + 1;
        }


    }
}

