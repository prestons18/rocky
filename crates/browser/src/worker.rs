use async_trait::async_trait;
use rocky_core::{Job, JobResult, JobError, JobWorker, Action, BrowserConfig};
use chromiumoxide::browser::{Browser, BrowserConfig as ChromeConfig};
use chromiumoxide::page::Page;
use chromiumoxide::browser::HeadlessMode;
use serde_json::json;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::Duration;
use futures::StreamExt;

pub struct BrowserWorker {
    browser_instances: Arc<Mutex<Vec<Browser>>>,
}

impl BrowserWorker {
    pub async fn new() -> Self {
        let browser_instances = Arc::new(Mutex::new(vec![]));
        Self { browser_instances }
    }

    async fn launch_browser(config: Option<BrowserConfig>) -> Result<Browser, JobError> {
        let headless = config.as_ref().map_or(true, |c| c.headless);
        let headless_mode = if headless { HeadlessMode::True } else { HeadlessMode::False };
        
        // Create a unique temporary directory for each browser instance to avoid SingletonLock conflicts
        let temp_dir = std::env::temp_dir().join(format!("chromium-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir)
            .map_err(|e| JobError::FetchError(format!("Failed to create temp dir: {}", e)))?;
        
        let chromium_cfg = ChromeConfig::builder()
            .headless_mode(headless_mode)
            .user_data_dir(temp_dir)
            .build()
            .map_err(|e| JobError::FetchError(format!("Browser launch failed: {}", e)))?;

        let (browser, mut handler) = Browser::launch(chromium_cfg)
            .await
            .map_err(|e| JobError::FetchError(format!("Browser launch failed: {}", e)))?;

        tokio::spawn(async move {
            while let Some(_) = handler.next().await {}
        });

        Ok(browser)
    }

    async fn get_browser(&self, config: Option<BrowserConfig>) -> Result<Browser, JobError> {
        let mut instances = self.browser_instances.lock().await;
        if instances.is_empty() {
            let b = Self::launch_browser(config.clone()).await?;
            instances.push(b);
        }
        // Browser doesn't implement Clone, so we need to return a reference or restructure
        // For now, launch a new browser each time
        Self::launch_browser(config).await
    }

    async fn perform_actions(&self, job: &Job, page: &Page) -> Result<serde_json::Value, JobError> {
        let mut output = serde_json::Map::new();

        for action in &job.actions {
            match action {
                Action::WaitFor { selector, timeout_ms } => {
                    // Wait for selector by polling with evaluate
                    let timeout = Duration::from_millis(*timeout_ms);
                    let start = std::time::Instant::now();
                    loop {
                        let js = format!("document.querySelector('{}') !== null", selector);
                        let result = page.evaluate(js).await
                            .map_err(|e| JobError::ActionError(format!("WaitFor eval failed: {}", e)))?;
                        
                        if let Some(val) = result.value() {
                            if val.as_bool() == Some(true) {
                                break;
                            }
                        }
                        
                        if start.elapsed() > timeout {
                            return Err(JobError::ActionError(format!("Timeout waiting for selector: {}", selector)));
                        }
                        tokio::time::sleep(Duration::from_millis(100)).await;
                    }
                    output.insert(format!("waitfor:{}", selector), json!(true));
                }
                Action::Extract { selector, attr } => {
                    let js = if let Some(a) = attr {
                        format!(
                            r#"Array.from(document.querySelectorAll("{}")).map(e => e.getAttribute("{}"))"#,
                            selector, a
                        )
                    } else {
                        format!(
                            r#"Array.from(document.querySelectorAll("{}")).map(e => e.textContent)"#,
                            selector
                        )
                    };

                    let eval = page.evaluate(js).await
                        .map_err(|e| JobError::ActionError(format!("Extract JS failed: {}", e)))?;

                    let values = eval.value().cloned().unwrap_or(json!([]));
                    output.insert(format!("extract:{}", selector), values);
                }
            }
        }

        Ok(serde_json::Value::Object(output))
    }
}

#[async_trait]
impl JobWorker for BrowserWorker {
    async fn execute(&self, job: &Job) -> Result<JobResult, JobError> {
        println!(
            "BrowserWorker: executing job {} on {:?}",
            job.id,
            job.browser_config.as_ref().map(|c| &c.browser_type)
        );

        let browser = self.get_browser(job.browser_config.clone()).await?;
        let page = browser.new_page("about:blank").await
            .map_err(|e| JobError::FetchError(format!("New page failed: {}", e)))?;

        page.goto(job.url.clone()).await
            .map_err(|e| JobError::FetchError(format!("Navigation failed: {}", e)))?;
        page.wait_for_navigation().await
            .map_err(|e| JobError::FetchError(format!("Navigation wait failed: {}", e)))?;

        let output = self.perform_actions(job, &page).await?;

        Ok(JobResult {
            job_id: job.id.clone(),
            success: true,
            output,
        })
    }
}