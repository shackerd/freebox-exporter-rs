use std::net::SocketAddr;

use log::{debug, info};

use crate::mappers::Mapper;

pub struct Server<'a> {
    port: u16,
    refresh_interval: u64,
    mapper: Mapper<'a>,
}

impl<'a> Server<'a> {
    pub fn new(port: u16, refresh_interval: u64, mapper: Mapper<'a>) -> Self {
        Self {
            port,
            refresh_interval,
            mapper,
        }
    }

    pub async fn run(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        debug!("initiating prometheus server");

        let addr_raw = format!("0.0.0.0:{}", self.port);

        info!("starting http server on {}", addr_raw);

        let addr: SocketAddr = match addr_raw.parse() {
            Err(e) => return Err(Box::new(e)),
            Ok(r) => r,
        };

        let exporter = match prometheus_exporter::start(addr) {
            Err(e) => return Err(Box::new(e)),
            Ok(r) => r,
        };

        let duration = std::time::Duration::from_secs(self.refresh_interval);

        let mut i = 0;

        match self.mapper.init_all().await {
            Err(e) => return Err(e),
            _ => {}
        };

        loop {
            debug!("fetching result from mapper maps");

            match self.mapper.set_all().await {
                Err(e) => return Err(e),
                _ => {}
            };

            i = i + 1;

            let _guard = exporter.wait_duration(duration);
        }
    }
}
