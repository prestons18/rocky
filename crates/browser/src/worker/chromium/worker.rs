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
use crate::shared::{TimeoutConfig, js};

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

    async fn check_captcha(&self, page: &chromiumoxide::page::Page) -> Result<(), JobError> {
        let js = js::build_js_call(js::element::DETECT_CAPTCHA, &[]);
        let result = page.evaluate(js).await
            .map_err(|e| JobError::script_error(format!("CAPTCHA detection failed: {}", e)))?;
        
        if let Some(value) = result.value() {
            if let Some(obj) = value.as_object() {
                // Log detection details for debugging
                let url = obj.get("url").and_then(|v| v.as_str()).unwrap_or("unknown");
                let detected = obj.get("detected").and_then(|v| v.as_bool()).unwrap_or(false);
                
                println!("      URL: {}", url);
                
                if detected {
                    let types = obj.get("types")
                        .and_then(|v| v.as_array())
                        .map(|arr| arr.iter()
                            .filter_map(|v| v.as_str())
                            .collect::<Vec<_>>()
                            .join(", "))
                        .unwrap_or_else(|| "unknown".to_string());
                    
                    let keywords = obj.get("keywords")
                        .and_then(|v| v.as_array())
                        .map(|arr| arr.iter()
                            .filter_map(|v| v.as_str())
                            .collect::<Vec<_>>()
                            .join(", "))
                        .unwrap_or_default();
                    
                    let page_title = obj.get("pageTitle")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown");
                    
                    let url = obj.get("url")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown");
                    
                    let title_match = obj.get("titleMatch").and_then(|v| v.as_bool()).unwrap_or(false);
                    let url_match = obj.get("urlMatch").and_then(|v| v.as_bool()).unwrap_or(false);
                    
                    let body_sample = obj.get("bodyTextSample")
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    
                    let message = if !keywords.is_empty() {
                        format!("CAPTCHA or consent page detected on '{}'", page_title)
                    } else if !types.is_empty() {
                        format!("CAPTCHA detected on '{}' (type: {})", page_title, types)
                    } else {
                        format!("CAPTCHA or verification page detected on '{}'", page_title)
                    };
                    
                    return Err(JobError::captcha_detected(message)
                        .with_context(json!({
                            "types": types,
                            "keywords": keywords,
                            "page_title": page_title,
                            "url": url,
                            "title_match": title_match,
                            "url_match": url_match,
                            "body_sample": body_sample
                        })));
                }
            }
        }
        
        Ok(())
    }

    async fn execute_actions(&self, job: &Job, page: &chromiumoxide::page::Page) -> Result<serde_json::Value, JobError> {
        let mut output = serde_json::Map::new();
        let fail_on_captcha = job.browser_config.as_ref().map_or(false, |c| c.fail_on_captcha);
        let action_handler = ActionHandler::new(self.timeout_config.clone(), fail_on_captcha);
        for (idx, action) in job.actions.iter().enumerate() {
            println!("  [{}] Action {}/{}: {:?}", job.id, idx + 1, job.actions.len(), action);
            
            let result = match action {
                Action::Scraping(a) => action_handler.handle_scraping(a, page, &mut output).await,
                Action::Browser(a) => action_handler.handle_browser(a, page, &mut output).await,
            };
            
            result.map_err(|e| {
                eprintln!("  [{}] ✗ Action {}/{} failed", job.id, idx + 1, job.actions.len());
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

            // Check for CAPTCHA if configured
            if job.browser_config.as_ref().map_or(false, |c| c.fail_on_captcha) {
                println!("  [{}] Checking for CAPTCHA...", job.id);
                self.check_captcha(&page).await?;
                println!("  [{}] ✓ No CAPTCHA detected", job.id);
            }

            let output = self.execute_actions(job, &page).await?;

            Ok(JobResult { 
                job_id: job.id.clone(), 
                success: true, 
                output 
            })
    }
}
