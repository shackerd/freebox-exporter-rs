use std::io::Write;

use async_trait::async_trait;
use log::info;

#[async_trait]
pub trait DryRunOutputWriter: Send + Sync {
    fn initialize(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
    fn push(&mut self, container: &str, section: &str, value: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
    fn flush(&mut self, container: &str, is_last: bool) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
    fn finalize(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
}

#[async_trait]
pub trait DryRunnable: Send + Sync {
    fn get_name(&self) -> Result<String, Box<dyn std::error::Error + Send + Sync>>;
    async fn dry_run(&mut self, writer: &mut dyn DryRunOutputWriter) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
    fn as_dry_runnable(&mut self) -> &mut dyn DryRunnable;
}

pub struct JsonFileOutputWriter<'a> {
    file: std::fs::File,
    output_path: &'a str,
    map: std::collections::HashMap<String, std::collections::HashMap<String, String>>, 
}

impl <'a> JsonFileOutputWriter<'a> {
    pub fn new(file: std::fs::File, output_path: &'a str) -> Self {
        Self { file, map: std::collections::HashMap::new(), output_path }
    }

    fn ensure_output_writable(&mut self) -> Result<&std::path::Path, Box<dyn std::error::Error + Send + Sync>> {
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
        Ok(path)
    }
}

impl <'a> DryRunOutputWriter for JsonFileOutputWriter<'a> {
    fn initialize(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {

        self.ensure_output_writable()?;

        self.file.write_all(b"{")?;
        Ok(())
    }

    fn push(&mut self, container: &str, section: &str, value: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        
        if !self.map.contains_key(container) {
            self.map.insert(container.to_string(), std::collections::HashMap::new());
        }

        let container_map = self.map.get_mut(container).unwrap();
        if !container_map.contains_key(section) {
            container_map.insert(section.to_string(), value.to_string());
        } else {
            let existing_value = container_map.get_mut(section).unwrap();
            existing_value.push_str(&format!("{}", value));
        }
        
        Ok(())
    }

    fn flush(&mut self, container: &str, is_last: bool) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        
        if let Some(container_map) = self.map.get(container) {

            self.file.write_all(format!("\"{}\": {{", container).as_bytes())?;

            let len = container_map.len();
            let mut i = 0;

            for (key, value) in container_map {

                i += 1;
                let comma = if i < len { "," } else { "" };

                if value.starts_with("{") || value.starts_with("[") || value.starts_with("\"") {
                    self.file.write_all(format!("\"{}\": {}{}", key, value, comma).as_bytes())?;
                    continue;
                }

                self.file.write_all(format!("\"{}\": \"{}\"{}", key, value, comma).as_bytes())?;
            }

            self.file.write_all(b"}")?;

        } else {

            self.file.write_all(format!("\"{}\": {{}}", container).as_bytes())?;
        }

        self.map.remove(container);

        if !is_last {
            self.file.write_all(b",")?;
        }

        self.file.flush()?;

        Ok(())
    }

    fn finalize(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.file.write_all(b"}")?;
        Ok(())
    }
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

        info!("running dry-run, outputting to {}", self.output_path);

        let mut writer = JsonFileOutputWriter::new(
            std::fs::OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .open(self.output_path)?,
            self.output_path,
        );

        writer.initialize()?;

        let len = self.runnables.len();
        let mut i = 0;

        for runnable in self.runnables.iter_mut() {
            i += 1;
            let name = &runnable.get_name().unwrap();
            println!("dry-running: {}", name);
            let textres = runnable.dry_run(&mut writer).await;

            match textres {
                Ok(_) => { 
                    let _ = writer.flush(name, i == len)?;                    
                },
                Err(e) => {
                    return Err(e); 
                }
            }            
        }

        writer.finalize()?;
        
        Ok(())
    }
}

