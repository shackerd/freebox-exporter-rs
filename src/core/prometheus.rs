use std::net::SocketAddr;

use log::debug;

use crate::mappers::Mapper;

pub struct Server {
    port: u16,
    refresh_interval: u64,
    mapper: Mapper
}

impl Server {
    pub fn new(port: u16, refresh_interval: u64, mapper: Mapper) -> Self {
        Self {
            port,
            refresh_interval,
            mapper
        }
    }

    pub async fn run(&mut self) -> Result<(), Box<dyn std::error::Error>> {

        debug!("initiating prometheus server");

        let addr_raw = format!("0.0.0.0:{}", self.port);
        let addr: SocketAddr = addr_raw.parse().expect("Cannot parse addr");
        let exporter = prometheus_exporter::start(addr).expect("Cannot start exporter");
        let duration = std::time::Duration::from_secs(self.refresh_interval);

        let mut i = 0;

        self.mapper.init_all().await?;

        loop {

            debug!("fetching result from mapper maps");

            self.mapper.set_all().await?;

            i = i + 1;

            let _guard = exporter.wait_duration(duration);
        }
    }
}

