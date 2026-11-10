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
            if let Err(e) = std::fs::remove_file(path) {
                return Err(Box::new(e));
            };
        }

        let mut file = match File::create(path).await {
            Err(e) => return Err(Box::new(e)),
            Ok(f) => f,
        };

        if let Err(e) = file.write_all(token.as_bytes()).await {
            if let Err(e) = file.shutdown().await {
                return Err(Box::new(e));
            };
            return Err(Box::new(e));
        }

        if let Err(e) = file.shutdown().await {
            return Err(Box::new(e));
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

        if let Err(e) = file.read_to_end(&mut buffer).await {
            return Err(Box::new(e));
        };

        let token = match String::from_utf8(buffer) {
            Err(e) => return Err(Box::new(e)),
            Ok(s) => s,
        };

        let trimmed_token = token.trim().to_string();

        Ok(trimmed_token)
    }
}
