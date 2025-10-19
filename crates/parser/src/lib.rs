use async_trait::async_trait;
use rocky_core::{Action, Job, JobError, JobResult, JobWorker, ScrapingAction, ErrorCategory};
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

    fn handle_scraping_action(
        &self,
        action: &ScrapingAction,
        document: &Html,
        output: &mut serde_json::Map<String, serde_json::Value>,
    ) -> Result<(), JobError> {
        match action {
            ScrapingAction::Fetch { .. } => {
                // Fetch is handled at the job level, not per action
            }
            ScrapingAction::WaitFor { selector, .. } => {
                // For static HTML parsing, we just check if the element exists
                let sel = Selector::parse(selector)
                    .map_err(|e| JobError::parsing_error(e.to_string()))?;
                let found = document.select(&sel).next().is_some();
                output.insert(format!("waitfor:{}", selector), json!(found));
            }
            ScrapingAction::Extract { selector, attr } => {
                let sel = Selector::parse(selector)
                    .map_err(|e| JobError::parsing_error(e.to_string()))?;
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
            ScrapingAction::ExtractMultiple { selector, attrs } => {
                let sel = Selector::parse(selector)
                    .map_err(|e| JobError::parsing_error(e.to_string()))?;
                let results: Vec<serde_json::Value> = document
                    .select(&sel)
                    .map(|el| {
                        let mut obj = serde_json::Map::new();
                        for attr in attrs {
                            let value = if attr == "text" {
                                el.text().collect::<Vec<_>>().join("")
                            } else {
                                el.value().attr(attr).unwrap_or("").to_string()
                            };
                            obj.insert(attr.clone(), json!(value));
                        }
                        serde_json::Value::Object(obj)
                    })
                    .collect();
                output.insert(format!("extract_multiple:{}", selector), json!(results));
            }
        }
        Ok(())
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
            .map_err(|e| JobError::fetch_error(e.to_string()))?
            .text()
            .await
            .map_err(|e| JobError::fetch_error(e.to_string()))?;

        let document = Html::parse_document(&html);
        let mut output = serde_json::Map::new();

        // Process each action sequentially
        for action in &job.actions {
            match action {
                Action::Scraping(scraping_action) => {
                    self.handle_scraping_action(scraping_action, &document, &mut output)?;
                }
                Action::Browser(_) => {
                    return Err(JobError::new(
                        ErrorCategory::Unknown,
                        "ParserWorker cannot execute browser actions. Use BrowserWorker instead."
                    ));
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