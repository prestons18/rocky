use rocky_core::JobResult;
use async_trait::async_trait;
use std::path::Path;

#[async_trait]
pub trait Storage: Send + Sync {
    async fn save_result(&self, result: &JobResult) -> anyhow::Result<()>;
}

pub struct JsonFileStorage {
    pub folder: String,
}

#[async_trait]
impl Storage for JsonFileStorage {
    async fn save_result(&self, result: &JobResult) -> anyhow::Result<()> {
        let path = Path::new(&self.folder).join(format!("{}.json", result.job_id));
        let data = serde_json::to_string_pretty(result)?;
        tokio::fs::write(path, data).await?;
        Ok(())
    }
}