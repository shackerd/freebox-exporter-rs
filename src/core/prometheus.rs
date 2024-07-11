use std::net::SocketAddr;

use crate::translators::Translator;

pub struct Server {
    port: u16,
    refresh_interval: u64,
    translator: Translator
}

impl Server {
    pub fn new(port: u16, refresh_interval: u64, translator: Translator) -> Self {
        Self {
            port,
            refresh_interval,
            translator
        }
    }

    pub async fn run(&self) -> Result<(), Box<dyn std::error::Error>> {

        let addr_raw = format!("0.0.0.0:{}", self.port);
        let addr: SocketAddr = addr_raw.parse().expect("Cannot parse addr");
        let exporter = prometheus_exporter::start(addr).expect("Cannot start exporter");
        let duration = std::time::Duration::from_secs(self.refresh_interval);

        let mut i = 0;

        loop {
            let _guard = exporter.wait_duration(duration);

            self.translator.set_all().await?;

            i = i + 1;
        }


    }
}

