use std::net::SocketAddr;

use clap::{command, Parser, Subcommand};
use configuration::get_configuration;
use prometheus_exporter::prometheus::{register_counter, register_gauge, register_histogram, register_int_counter_vec};

mod common;
mod authenticator;
mod discovery;
mod prometheus;
mod configuration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {

    let cli = Cli::parse();

    let conf_path: &str =
        match &cli.configuration_file {
            Some(c) => { &c },
            None => { "config.toml" }
        };

    let conf = get_configuration(conf_path.to_string()).await?;

    match &cli.command {
        Command::Register { pooling_interval } => {
            let interval =
                match &pooling_interval {
                    Some(i) => { *i },
                    None => { 6 }
                };

            register(conf.api.host, interval).await?;
        } ,
        Command::Serve { port } => {
            let serve_port =
                match &port {
                    Some(p) => { *p },
                    None => { conf.core.port }
                };

            serve(conf.api.host, serve_port).await?;
        },
        Command::Revoke => {
            todo!()
        }
    }

    Ok(())
}

async fn register(freebox_host: String, interval: u64) -> Result<(), Box<dyn std::error::Error>> {
    let api_url = discovery::get_api_url(freebox_host.as_str(), true).await?;

    let authenticator =
        authenticator::Authenticator::new(api_url.to_owned());

    authenticator.register(interval).await?;

    Ok(())
}

async fn serve(freebox_host: String, port: u16) -> Result<(), Box<dyn std::error::Error>> {

    let api_url = discovery::get_api_url(freebox_host.as_str(), true).await?;
    let authenticator =
        authenticator::Authenticator::new(api_url.to_owned());

    // server startup procedure
    let client = authenticator.login().await?;

    let addr_raw = format!("0.0.0.0:{}", port);
    let addr: SocketAddr = addr_raw.parse().expect("Cannot parse addr");
    let exporter = prometheus_exporter::start(addr).expect("Cannot start exporter");
    let duration = std::time::Duration::from_millis(5000);
    let metric1 = register_gauge!("bytes_down", "bytes_down").expect("cannot create test gauge");

    let mut i = 0;
    loop {
        let guard = exporter.wait_duration(duration);
        let connection = client.test().await.unwrap();

        metric1.set(connection.result.bytes_down as f64);

        i = i + 1;

        if i > 100 { // dummy code
            break;
        }
    }

    Ok(())
}

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Command,
    #[arg(short, long)]
    configuration_file: Option<String>
}

#[derive(Subcommand)]
enum Command {

    Register { pooling_interval: Option<u64>},
    Serve { port: Option<u16>},
    Revoke
}