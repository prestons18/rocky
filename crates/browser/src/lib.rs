use rocky_core::{Job, JobResult, JobError, JobWorker};
use async_trait::async_trait;
use serde_json::json;
use tokio::time::{sleep, Duration};

pub struct BrowserWorker;

impl BrowserWorker {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl JobWorker for BrowserWorker {
    async fn execute(&self, job: &Job) -> Result<JobResult, JobError> {
        println!("BrowserWorker: executing job {}", job.id);

        // Simulate async browser work
        sleep(Duration::from_millis(500)).await;

        // Stub result
        Ok(JobResult {
            job_id: job.id.clone(),
            success: true,
            output: json!({
                "browser_stub": true,
                "url": job.url,
                "actions_count": job.actions.len()
            }),
        })
    }
}