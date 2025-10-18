use async_trait::async_trait;
use rocky_core::JobResult;
use std::path::Path;
use anyhow::Result;

#[async_trait]
pub trait Storage: Send + Sync {
    async fn save_result(&self, result: &JobResult) -> Result<()>;
}

pub struct JsonFileStorage {
    pub folder: String,
}

impl JsonFileStorage {
    pub fn new(folder: &str) -> Self {
        std::fs::create_dir_all(folder).ok(); // ensure folder exists
        Self { folder: folder.to_string() }
    }
}

#[async_trait]
impl Storage for JsonFileStorage {
    async fn save_result(&self, result: &JobResult) -> Result<()> {
        let path = Path::new(&self.folder).join(format!("{}.json", result.job_id));
        let data = serde_json::to_string_pretty(result)?;
        tokio::fs::write(path, data).await?;
        Ok(())
    }
}