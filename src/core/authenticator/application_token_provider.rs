use std::path::Path;

use async_trait::async_trait;
use log::error;
use mockall::automock;
use tokio::{
    fs::File,
    io::{AsyncReadExt, AsyncWriteExt},
};

#[automock]
#[async_trait]
pub trait ApplicationTokenProvider: Send + Sync {
    async fn store(&self, token: String) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
    async fn get(&self) -> Result<String, Box<dyn std::error::Error + Send + Sync>>;
}

#[derive(Clone)]
pub struct FileSystemProvider {
    path: String,
}

impl FileSystemProvider {
    pub fn new(data_dir: String) -> Self {
        let path = FileSystemProvider::get_token_file_path(data_dir);
        Self { path }
    }

    pub fn get_token_file_path(data_dir: String) -> String {
        let sep = if cfg!(windows) { '\\' } else { '/' };
        format!("{}{}{}", data_dir, sep, "token.dat")
    }
}

#[async_trait]
impl ApplicationTokenProvider for FileSystemProvider {
    async fn store(&self, token: String) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let path = Path::new(&self.path);

        if path.exists() {
            match std::fs::remove_file(path) {
                Err(e) => return Err(Box::new(e)),
                _ => {}
            };
        }

        let mut file = match File::create(path).await {
            Err(e) => return Err(Box::new(e)),
            Ok(f) => f,
        };

        match file.write_all(token.as_bytes()).await {
            Err(e) => {
                match file.shutdown().await {
                    Err(e) => return Err(Box::new(e)),
                    _ => {}
                };
                return Err(Box::new(e));
            }
            _ => {}
        }

        match file.shutdown().await {
            Err(e) => return Err(Box::new(e)),
            _ => {}
        };

        Ok(())
    }

    async fn get(&self) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let path = Path::new(self.path.as_str());

        if !path.exists() {
            error!(
                "file does not exist {}, did you registered the application? See register command",
                self.path
            );
            return Err(Box::new(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("file does not exist {}", self.path),
            )));
        }

        let mut file = match File::open(&self.path).await {
            Err(e) => return Err(Box::new(e)),
            Ok(f) => f,
        };

        let mut buffer = vec![];

        match file.read_to_end(&mut buffer).await {
            Err(e) => return Err(Box::new(e)),
            _ => {}
        };

        let token = match String::from_utf8(buffer) {
            Err(e) => return Err(Box::new(e)),
            Ok(s) => s,
        };

        let trimmed_token = token.trim().to_string();

        Ok(trimmed_token)
    }
}
