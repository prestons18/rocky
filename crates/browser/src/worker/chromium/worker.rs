use async_trait::async_trait;
use chromiumoxide::browser::{Browser, BrowserConfig as ChromeConfig};
use chromiumoxide::browser::HeadlessMode;
use futures::StreamExt;
use rocky_core::{Job, JobResult, JobError, JobWorker, Action, BrowserConfig};
use serde_json::json;
use std::sync::Arc;
use tokio::sync::Mutex;

use super::actions::ActionHandler;
use super::wait::WaitStrategy;
use crate::shared::TimeoutConfig;

pub struct ChromiumWorker {
    browser_instances: Arc<Mutex<Vec<Browser>>>,
    timeout_config: TimeoutConfig,
}

impl ChromiumWorker {
    pub fn new() -> Self {
        Self::with_config(TimeoutConfig::default())
    }

    pub fn with_config(timeout_config: TimeoutConfig) -> Self {
        Self {
            browser_instances: Arc::new(Mutex::new(vec![])),
            timeout_config,
        }
    }

    async fn launch(config: Option<BrowserConfig>) -> Result<Browser, JobError> {
        let headless = config.as_ref().map_or(true, |c| c.headless);
        let temp_dir = std::env::temp_dir().join(format!("chromium-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir)
            .map_err(|e| JobError::browser_error(format!("Failed to create temp dir: {}", e)))?;
        
        let mut builder = ChromeConfig::builder()
            .headless_mode(if headless { HeadlessMode::True } else { HeadlessMode::False })
            .user_data_dir(temp_dir);

        if let Some(cfg) = config {
            if let (Some(w), Some(h)) = (cfg.viewport_width, cfg.viewport_height) {
                builder = builder.window_size(w, h);
            }
        }

        let chrome_cfg = builder.build()
            .map_err(|e| JobError::browser_error(format!("Config failed: {}", e)))?;

        let (browser, mut handler) = Browser::launch(chrome_cfg).await
            .map_err(|e| JobError::browser_error(format!("Launch failed: {}", e)))?;

        tokio::spawn(async move { while handler.next().await.is_some() {} });
        Ok(browser)
    }

    async fn execute_actions(&self, job: &Job, page: &chromiumoxide::page::Page) -> Result<serde_json::Value, JobError> {
        let mut output = serde_json::Map::new();
        let action_handler = ActionHandler::new(self.timeout_config.clone());
        for (idx, action) in job.actions.iter().enumerate() {
            println!("  [{}] Action {}/{}: {:?}", job.id, idx + 1, job.actions.len(), action);
            
            let result = match action {
                Action::Scraping(a) => action_handler.handle_scraping(a, page, &mut output).await,
                Action::Browser(a) => action_handler.handle_browser(a, page, &mut output).await,
            };
            
            result.map_err(|e| {
                eprintln!("  [{}] ✗ Action {}/{} failed: {}", job.id, idx + 1, job.actions.len(), e);
                e
            })?;
            
            println!("  [{}] ✓ Action {}/{} completed", job.id, idx + 1, job.actions.len());
        }
    
        Ok(json!(output))
    }
}

#[async_trait]
impl JobWorker for ChromiumWorker {
    async fn execute(&self, job: &Job) -> Result<JobResult, JobError> {
        println!("ChromiumWorker: executing job {}", job.id);
        let browser = Self::launch(job.browser_config.clone()).await?;
            let page = browser.new_page("about:blank").await
                .map_err(|e| JobError::browser_error(format!("New page failed: {}", e)))?;

            println!("  [{}] Navigating to {}...", job.id, job.url);
            page.goto(job.url.clone()).await
                .map_err(|e| JobError::navigation_error(format!("Navigation failed: {}", e)))?;
            
            let wait_strategy = WaitStrategy::new(self.timeout_config.clone());
            wait_strategy.wait_for_stable(&page, self.timeout_config.page_stable.as_millis() as u64).await?;
            println!("  [{}] Page loaded and stabilized", job.id);

            let output = self.execute_actions(job, &page).await?;

            Ok(JobResult { 
                job_id: job.id.clone(), 
                success: true, 
                output 
            })
    }
}
