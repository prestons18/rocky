use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Serialize, Deserialize)]
pub enum Action {
    WaitFor {
        selector: String,
        timeout_ms: u64,
    },
    Extract {
        selector: String,
        attr: Option<String>,
    },
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BrowserConfig {
    pub browser_type: BrowserType,
    pub headless: bool,
    pub viewport_width: Option<u32>,
    pub viewport_height: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum BrowserType {
    Chromium,
    Firefox,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Job {
    pub id: String,
    pub url: String,
    pub use_browser: bool,
    pub actions: Vec<Action>,
    pub browser_config: Option<BrowserConfig>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct JobResult {
    pub job_id: String,
    pub success: bool,
    pub output: serde_json::Value,
}

#[derive(Debug, Error)]
pub enum JobError {
    #[error("Failed to fetch page: {0}")]
    FetchError(String),

    #[error("Action failed: {0}")]
    ActionError(String),
}

#[async_trait]
pub trait JobWorker: Send + Sync {
    async fn execute(&self, job: &Job) -> Result<JobResult, JobError>;
}
