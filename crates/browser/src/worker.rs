use async_trait::async_trait;
use rocky_core::{Job, JobResult, JobError, JobWorker, Action, BrowserConfig, ScrapingAction, BrowserAction, ScrollTarget};
use chromiumoxide::browser::{Browser, BrowserConfig as ChromeConfig};
use chromiumoxide::page::Page;
use chromiumoxide::browser::HeadlessMode;
use chromiumoxide::cdp::browser_protocol::page::{CaptureScreenshotParams, CaptureScreenshotFormat};
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
                Action::Scraping(scraping_action) => {
                    self.handle_scraping_action(scraping_action, page, &mut output).await?;
                }
                Action::Browser(browser_action) => {
                    self.handle_browser_action(browser_action, page, &mut output).await?;
                }
            }
        }

        Ok(serde_json::Value::Object(output))
    }

    async fn handle_scraping_action(
        &self,
        action: &ScrapingAction,
        page: &Page,
        output: &mut serde_json::Map<String, serde_json::Value>,
    ) -> Result<(), JobError> {
        match action {
            ScrapingAction::WaitFor { selector, timeout_ms } => {
                let timeout = Duration::from_millis(*timeout_ms);
                let start = std::time::Instant::now();
                let selector_json = serde_json::to_string(selector)
                    .map_err(|e| JobError::ActionError(format!("Failed to serialize selector: {}", e)))?;
                
                loop {
                    let js = format!("document.querySelector({}) !== null", selector_json);
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
            ScrapingAction::Extract { selector, attr } => {
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
            ScrapingAction::ExtractMultiple { selector, attrs } => {
                let attrs_json = serde_json::to_string(attrs)
                    .map_err(|e| JobError::ActionError(format!("Failed to serialize attrs: {}", e)))?;
                
                let js = format!(
                    r#"
                    Array.from(document.querySelectorAll("{}")).map(e => {{
                        const result = {{}};
                        const attrs = {};
                        attrs.forEach(attr => {{
                            if (attr === 'text') {{
                                result[attr] = e.textContent;
                            }} else {{
                                result[attr] = e.getAttribute(attr) || '';
                            }}
                        }});
                        return result;
                    }})
                    "#,
                    selector, attrs_json
                );

                let eval = page.evaluate(js).await
                    .map_err(|e| JobError::ActionError(format!("ExtractMultiple JS failed: {}", e)))?;

                let values = eval.value().cloned().unwrap_or(json!([]));
                output.insert(format!("extract_multiple:{}", selector), values);
            }
        }
        Ok(())
    }

    async fn handle_browser_action(
        &self,
        action: &BrowserAction,
        page: &Page,
        output: &mut serde_json::Map<String, serde_json::Value>,
    ) -> Result<(), JobError> {
        match action {
            BrowserAction::Click { selector, timeout_ms } => {
                // Wait for element to be clickable
                let timeout = Duration::from_millis(*timeout_ms);
                let start = std::time::Instant::now();
                let selector_json = serde_json::to_string(selector)
                    .map_err(|e| JobError::ActionError(format!("Failed to serialize selector: {}", e)))?;
                
                loop {
                    let js = format!("document.querySelector({}) !== null", selector_json);
                    let result = page.evaluate(js).await
                        .map_err(|e| JobError::ActionError(format!("Click wait failed: {}", e)))?;
                    
                    if let Some(val) = result.value() {
                        if val.as_bool() == Some(true) {
                            break;
                        }
                    }
                    
                    if start.elapsed() > timeout {
                        return Err(JobError::ActionError(format!("Timeout waiting for clickable element: {}", selector)));
                    }
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }

                // Click the element
                let click_js = format!("document.querySelector({}).click()", selector_json);
                page.evaluate(click_js).await
                    .map_err(|e| JobError::ActionError(format!("Click failed: {}", e)))?;
                
                output.insert(format!("click:{}", selector), json!(true));
            }
            BrowserAction::Type { selector, text, clear_first } => {
                // Properly escape strings by using JSON serialization
                let selector_json = serde_json::to_string(selector)
                    .map_err(|e| JobError::ActionError(format!("Failed to serialize selector: {}", e)))?;
                let text_json = serde_json::to_string(text)
                    .map_err(|e| JobError::ActionError(format!("Failed to serialize text: {}", e)))?;
                
                let js = if *clear_first {
                    format!(
                        r#"
                        {{
                            const el = document.querySelector({});
                            el.value = '';
                            el.focus();
                            el.value = {};
                            el.dispatchEvent(new Event('input', {{ bubbles: true }}));
                        }}
                        "#,
                        selector_json, text_json
                    )
                } else {
                    format!(
                        r#"
                        {{
                            const el = document.querySelector({});
                            el.focus();
                            el.value += {};
                            el.dispatchEvent(new Event('input', {{ bubbles: true }}));
                        }}
                        "#,
                        selector_json, text_json
                    )
                };

                page.evaluate(js).await
                    .map_err(|e| JobError::ActionError(format!("Type failed: {}", e)))?;
                
                output.insert(format!("type:{}", selector), json!(text));
            }
            BrowserAction::PressKey { key } => {
                // Simulate key press using keyboard events
                let key_json = serde_json::to_string(key)
                    .map_err(|e| JobError::ActionError(format!("Failed to serialize key: {}", e)))?;
                
                let js = format!(
                    r#"
                    document.dispatchEvent(new KeyboardEvent('keydown', {{ key: {} }}));
                    document.dispatchEvent(new KeyboardEvent('keyup', {{ key: {} }}));
                    "#,
                    key_json, key_json
                );

                page.evaluate(js).await
                    .map_err(|e| JobError::ActionError(format!("PressKey failed: {}", e)))?;
                
                output.insert("press_key".to_string(), json!(key));
            }
            BrowserAction::Scroll { target } => {
                let js = match target {
                    ScrollTarget::Element { selector } => {
                        let selector_json = serde_json::to_string(selector)
                            .map_err(|e| JobError::ActionError(format!("Failed to serialize selector: {}", e)))?;
                        format!("document.querySelector({}).scrollIntoView({{ behavior: 'smooth' }})", selector_json)
                    }
                    ScrollTarget::Position { x, y } => {
                        format!("window.scrollTo({}, {})", x, y)
                    }
                    ScrollTarget::Bottom => {
                        "window.scrollTo(0, document.body.scrollHeight)".to_string()
                    }
                    ScrollTarget::Top => {
                        "window.scrollTo(0, 0)".to_string()
                    }
                };

                page.evaluate(js).await
                    .map_err(|e| JobError::ActionError(format!("Scroll failed: {}", e)))?;
                
                output.insert("scroll".to_string(), json!(true));
            }
            BrowserAction::Screenshot { path, full_page } => {
                let mut screenshot_params = CaptureScreenshotParams::builder()
                    .format(CaptureScreenshotFormat::Png);
                
                if *full_page {
                    screenshot_params = screenshot_params.capture_beyond_viewport(true);
                }

                let screenshot_bytes = page.screenshot(screenshot_params.build()).await
                    .map_err(|e| JobError::ActionError(format!("Screenshot failed: {}", e)))?;

                // Save to file
                tokio::fs::write(path, &screenshot_bytes).await
                    .map_err(|e| JobError::ActionError(format!("Failed to save screenshot: {}", e)))?;
                
                output.insert("screenshot".to_string(), json!(path));
            }
            BrowserAction::Hover { selector } => {
                let selector_json = serde_json::to_string(selector)
                    .map_err(|e| JobError::ActionError(format!("Failed to serialize selector: {}", e)))?;
                
                let js = format!(
                    r#"
                    {{
                        const el = document.querySelector({});
                        const event = new MouseEvent('mouseover', {{ bubbles: true }});
                        el.dispatchEvent(event);
                    }}
                    "#,
                    selector_json
                );

                page.evaluate(js).await
                    .map_err(|e| JobError::ActionError(format!("Hover failed: {}", e)))?;
                
                output.insert(format!("hover:{}", selector), json!(true));
            }
            BrowserAction::Select { selector, value } => {
                let selector_json = serde_json::to_string(selector)
                    .map_err(|e| JobError::ActionError(format!("Failed to serialize selector: {}", e)))?;
                let value_json = serde_json::to_string(value)
                    .map_err(|e| JobError::ActionError(format!("Failed to serialize value: {}", e)))?;
                
                let js = format!(
                    r#"
                    {{
                        const el = document.querySelector({});
                        el.value = {};
                        el.dispatchEvent(new Event('change', {{ bubbles: true }}));
                    }}
                    "#,
                    selector_json, value_json
                );

                page.evaluate(js).await
                    .map_err(|e| JobError::ActionError(format!("Select failed: {}", e)))?;
                
                output.insert(format!("select:{}", selector), json!(value));
            }
            BrowserAction::Navigate { url } => {
                page.goto(url).await
                    .map_err(|e| JobError::ActionError(format!("Navigate failed: {}", e)))?;
                page.wait_for_navigation().await
                    .map_err(|e| JobError::ActionError(format!("Navigation wait failed: {}", e)))?;
                
                output.insert("navigate".to_string(), json!(url));
            }
            BrowserAction::ExecuteScript { script } => {
                let result = page.evaluate(script.clone()).await
                    .map_err(|e| JobError::ActionError(format!("ExecuteScript failed: {}", e)))?;
                
                let value = result.value().cloned().unwrap_or(json!(null));
                output.insert("execute_script".to_string(), value);
            }
            BrowserAction::SetCookie { name, value, domain } => {
                let domain_str = domain.as_ref().map(|d| d.as_str()).unwrap_or("");
                let js = format!(
                    r#"document.cookie = "{}={}; domain={}; path=/""#,
                    name, value, domain_str
                );

                page.evaluate(js).await
                    .map_err(|e| JobError::ActionError(format!("SetCookie failed: {}", e)))?;
                
                output.insert(format!("set_cookie:{}", name), json!(value));
            }
            BrowserAction::WaitForNavigation { timeout_ms } => {
                let timeout = Duration::from_millis(*timeout_ms);
                tokio::time::timeout(timeout, page.wait_for_navigation())
                    .await
                    .map_err(|_| JobError::ActionError("Navigation timeout".to_string()))?
                    .map_err(|e| JobError::ActionError(format!("Navigation wait failed: {}", e)))?;
                
                output.insert("wait_for_navigation".to_string(), json!(true));
            }
        }
        Ok(())
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