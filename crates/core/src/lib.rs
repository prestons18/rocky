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

#[derive(Debug, Serialize, Deserialize)]
pub struct Job {
    pub id: String,
    pub url: String,
    pub use_browser: bool,
    pub actions: Vec<Action>,
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

pub trait JobWorker {
    fn execute(&self, job: &Job) -> Result<JobResult, JobError>;
}
