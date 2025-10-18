use async_trait::async_trait;
use rocky_core::{Action, Job, JobError, JobResult, JobWorker};
use reqwest::Client;
use scraper::{Html, Selector};
use serde_json::json;

pub struct ParserWorker {
    client: Client,
}

impl ParserWorker {
    pub fn new() -> Self {
        Self { client: Client::new() }
    }
}

#[async_trait]
impl JobWorker for ParserWorker {
    async fn execute(&self, job: &Job) -> Result<JobResult, JobError> {
        // Fetch page
        let html = self.client
            .get(&job.url)
            .send()
            .await
            .map_err(|e| JobError::FetchError(e.to_string()))?
            .text()
            .await
            .map_err(|e| JobError::FetchError(e.to_string()))?;

        let document = Html::parse_document(&html);
        let mut output = serde_json::Map::new();

        // Process each action sequentially
        for action in &job.actions {
            match action {
                Action::WaitFor { selector, .. } => {
                    let sel = Selector::parse(selector)
                        .map_err(|e| JobError::ActionError(e.to_string()))?;
                    let found = document.select(&sel).next().is_some();
                    output.insert(format!("waitfor:{}", selector), json!(found));
                }
                Action::Extract { selector, attr } => {
                    let sel = Selector::parse(selector)
                        .map_err(|e| JobError::ActionError(e.to_string()))?;
                    let values: Vec<String> = document
                        .select(&sel)
                        .map(|el| {
                            if let Some(a) = attr {
                                el.value().attr(a).unwrap_or("").to_string()
                            } else {
                                el.text().collect::<Vec<_>>().join("")
                            }
                        })
                        .collect();
                    output.insert(format!("extract:{}", selector), json!(values));
                }
            }
        }

        Ok(JobResult {
            job_id: job.id.clone(),
            success: true,
            output: serde_json::Value::Object(output),
        })
    }
}