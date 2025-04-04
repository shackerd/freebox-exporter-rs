use std::io::Write;

use async_trait::async_trait;
use log::info;

#[async_trait]
pub trait DryRunnable: Send + Sync{
    fn get_name(&self) -> Result<String, Box<dyn std::error::Error>>;
    async fn dry_run(&mut self) -> Result<String, Box<dyn std::error::Error>>;
    fn coerce(&mut self) -> &mut dyn DryRunnable;
}

pub struct DryRunner<'a>{
    runnables: Vec<&'a mut dyn DryRunnable>,
    output_path: &'a str,
}

impl <'a> DryRunner<'a> {
    pub fn new(runnables: Vec<&'a mut dyn DryRunnable>, output_path: &'a str) -> Self {
        Self {
            runnables,
            output_path
        }
    }

    pub async fn run(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!("running dry run, outputting to {}", self.output_path);
        // ensure filepath is writable
        let path = std::path::Path::new(self.output_path);
        if path.exists() {
            if path.is_dir() {
                return Err(Box::new(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "output path is a directory",
                )));
            }
        } else {
            if let Some(parent) = path.parent() {
                if !parent.exists() {
                    return Err(Box::new(std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        "output path parent directory does not exist",
                    )));
                }
            }
        }
        
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(path)?; 
        
        file.write_all(b"{\n")?;

        let mut i = 0;
        let len = self.runnables.len();
        for runnable in self.runnables.iter_mut() {
            let name = &runnable.get_name().unwrap();
            let textres = runnable.dry_run().await;

            match textres {
                Ok(res) => {
                    // write to file
                    file.write_all(format!("\"{}\":", name).as_bytes())?;
                    if res.is_empty() {
                        file.write_all(b"\"No output\"")?;
                    } else {
                        file.write_all(format!("{}", res).as_bytes())?;
                    }
                    i = i + 1;
                    if i == len {
                        file.write_all(b"\n")?;
                    } else {
                        file.write_all(b",\n")?;
                    }                    
                },
                Err(e) => {
                    print!("{}: ", name);
                    println!("Error: {}", e);
                }
            }
        }
        file.write_all(b"}")?;
        Ok(())
    }
}

/*
pub async fn dump_json(&mut self, output_file: Option<&str>) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Fetch switch port statuses
        let port_statuses = self.get_ports_status().await?;
        let mut output = String::new();

        // Serialize and append the port statuses to the output
        let port_statuses_json = serde_json::to_string_pretty(&port_statuses)?;
        output.push_str(&format!("Switch Port Statuses:\n{}\n\n", port_statuses_json));

        // Fetch and serialize stats for each port
        for port_status in &port_statuses {
            let port_id = port_status.id.unwrap_or_default();
            let stats = self.get_port_stats(port_status).await?;
            let stats_json = serde_json::to_string_pretty(&stats)?;
            output.push_str(&format!("Stats for Port {}:\n{}\n\n", port_id, stats_json));
        }

        // Write to stdout or file
        if let Some(file_path) = output_file {
            let mut file = File::create(file_path)?;
            file.write_all(output.as_bytes())?;
            println!("JSON content dumped to file: {}", file_path);
        } else {
            println!("{}", output);
        }

        Ok(())
    } */
