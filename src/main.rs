use clap::{command, Parser, Subcommand};
use translators::Translator;
use core::{authenticator, configuration::{get_configuration, Configuration}, discovery, prometheus::{self}};

mod core;
mod translators;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {

    let cli = Cli::parse();

    let conf_path: &str =
        match &cli.configuration_file {
            Some(c) => { &c },
            None => { "config.toml" }
        };

    let conf = get_configuration(conf_path.to_string()).await?;
    conf.assert_data_dir_permissions()?;

    match &cli.command {
        Command::Register { pooling_interval } => {
            let interval =
                match &pooling_interval {
                    Some(i) => { *i },
                    None => { 6 }
                };

            register(conf, interval).await?;
        } ,
        Command::Serve { port } => {
            let serve_port =
                match &port {
                    Some(p) => { *p },
                    None => { conf.core.port.unwrap() }
                };

            serve(conf, serve_port).await?;
        },
        Command::Revoke => {
            todo!()
        }
    }

    Ok(())
}

async fn register(conf: Configuration, interval: u64) -> Result<(), Box<dyn std::error::Error>> {

    let api_url =
        match conf.api.mode.expect("Please specify freebox mode").as_str() {
            "router" => { discovery::get_api_url(discovery::DEFAULT_FBX_HOST).await? },
            "bridge" => { discovery::get_static_api_url().unwrap() }
            _ => { panic!("Unrecognized freebox mode") }
        };

    println!("Now using api url: {api_url}");

    let authenticator =
        authenticator::Authenticator::new(api_url.to_owned(), conf.core.data_directory.unwrap());

    match authenticator.register(interval).await {
        Ok(_) => {
            println!("Successfully registered application");
        },
        Err(e) => panic!("{e:#?}")
    }

    Ok(())
}

async fn serve(conf: Configuration, port: u16) -> Result<(), Box<dyn std::error::Error>> {

    let api_url =
        match conf.api.mode.expect("Please specify freebox mode").as_str() {
            "router" => { discovery::get_api_url(discovery::DEFAULT_FBX_HOST).await? },
            "bridge" => { discovery::get_static_api_url().unwrap() }
            _ => { panic!("Unrecognized freebox mode") }
        };

    println!("Now using api url: {api_url}");

    let authenticator =
        authenticator::Authenticator::new(api_url.to_owned(), conf.core.data_directory.unwrap());

    let factory = authenticator.login().await?;
    let translator = Translator::new(factory, conf.publish);
    let server = prometheus::Server::new(port, conf.api.refresh.unwrap_or_else(|| 5), translator);

    server.run().await?;

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