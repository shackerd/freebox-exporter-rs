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

        let lock = exporter.update_lock();
        let duration = std::time::Duration::from_secs(self.refresh_interval);

        self.mapper.init_all().await?;

        loop {
            debug!("fetching result from mapper maps");

            self.mapper.set_all(&lock).await?;

            let _guard = exporter.wait_duration(duration);
        }
    }
}
