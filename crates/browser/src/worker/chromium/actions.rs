use chromiumoxide::page::Page;
use chromiumoxide::cdp::browser_protocol::page::{CaptureScreenshotParams, CaptureScreenshotFormat};
use rocky_core::{JobError, ScrapingAction, BrowserAction, ScrollTarget};
use serde_json::{json, Map, Value};
use std::time::Duration;
use tokio::time::sleep;
use crate::shared::{js, to_job_error, TimeoutConfig};
use super::wait::WaitStrategy;

pub struct ActionHandler {
    wait_strategy: WaitStrategy,
}

impl ActionHandler {
    pub fn new(config: TimeoutConfig) -> Self {
        Self {
            wait_strategy: WaitStrategy::new(config),
        }
    }

    async fn scroll_to_element(&self, page: &Page, selector: &str) -> Result<(), JobError> {
        let js = js::build_js_call(js::element::SCROLL_INTO_VIEW, &[json!(selector), json!("center")]);
        page.evaluate(js).await
            .map_err(|e| to_job_error(e, "Scroll"))?;
        sleep(Duration::from_millis(300)).await;
        Ok(())
    }

    async fn scroll(&self, page: &Page, target: &ScrollTarget) -> Result<(), JobError> {
        let js = match target {
            ScrollTarget::Element { selector } => {
                js::build_js_call(js::element::SCROLL_INTO_VIEW, &[json!(selector), json!("center")])
            }
            ScrollTarget::Position { x, y } => format!("window.scrollTo({},{})", x, y),
            ScrollTarget::Bottom => "window.scrollTo(0,document.body.scrollHeight)".to_string(),
            ScrollTarget::Top => "window.scrollTo(0,0)".to_string(),
        };
        
        page.evaluate(js).await
            .map_err(|e| to_job_error(e, "Scroll"))?;
        sleep(Duration::from_millis(500)).await;
        Ok(())
    }

    pub async fn handle_scraping(
        &self,
        action: &ScrapingAction,
        page: &Page,
        output: &mut Map<String, Value>,
    ) -> Result<(), JobError> {
        match action {
            ScrapingAction::Fetch { .. } => Ok(()),
            ScrapingAction::WaitFor { selector, timeout_ms } => {
                self.wait_strategy.wait_for_element(page, selector, *timeout_ms, false).await?;
                output.insert(format!("waitfor:{}", selector), json!(true));
                Ok(())
            }
            ScrapingAction::Extract { selector, attr } => {
                let js = if let Some(a) = attr {
                    js::build_js_call(js::element::EXTRACT_ATTR, &[json!(selector), json!(a)])
                } else {
                    js::build_js_call(js::element::EXTRACT_TEXT, &[json!(selector)])
                };
                
                let result = page.evaluate(js).await
                    .map_err(|e| JobError::script_error(format!("Extract failed: {}", e)))?;
                
                output.insert(format!("extract:{}", selector), result.value().cloned().unwrap_or(json!([])));
                Ok(())
            }
            ScrapingAction::ExtractMultiple { selector, attrs } => {
                let js = js::build_js_call(js::element::EXTRACT_MULTIPLE, &[json!(selector), json!(attrs)]);
                let result = page.evaluate(js).await
                    .map_err(|e| JobError::script_error(format!("ExtractMultiple failed: {}", e)))?;
                
                output.insert(format!("extract_multiple:{}", selector), result.value().cloned().unwrap_or(json!([])));
                Ok(())
            }
        }
    }
    
    pub async fn handle_browser(
        &self,
        action: &BrowserAction,
        page: &Page,
        output: &mut Map<String, Value>,
    ) -> Result<(), JobError> {
        match action {
            BrowserAction::Click { selector, timeout_ms } => {
                self.wait_strategy.wait_for_element(page, selector, *timeout_ms, true).await?;
                self.scroll_to_element(page, selector).await?;
                
                let js = js::build_js_call(js::element::SAFE_CLICK, &[json!(selector)]);
                page.evaluate(js).await
                    .map_err(|e| JobError::script_error(format!("Click failed: {}", e)))?;
                
                sleep(Duration::from_millis(300)).await;
                output.insert(format!("click:{}", selector), json!(true));
                Ok(())
            }
            BrowserAction::Type { selector, text, clear_first } => {
                self.wait_strategy.wait_for_element(page, selector, 10000, false).await?;
                
                let js = js::build_js_call(js::element::TYPE_TEXT, &[json!(selector), json!(text), json!(clear_first)]);
                page.evaluate(js).await
                    .map_err(|e| JobError::script_error(format!("Type failed: {}", e)))?;
                
                sleep(Duration::from_millis(200)).await;
                output.insert(format!("type:{}", selector), json!(text));
                Ok(())
            }
            BrowserAction::PressKey { key } => {
                // Special handling for Enter key - try to submit the active element's form
                if key.to_lowercase() == "enter" {
                    let js = r#"
                        (() => {
                            const el = document.activeElement;
                            if (!el) return { success: false, error: 'No active element' };
                            
                            // Try form submission first
                            const form = el.closest('form');
                            if (form) {
                                form.submit();
                                return { success: true, method: 'form.submit' };
                            }
                            
                            // Dispatch proper keyboard events
                            const enterDown = new KeyboardEvent('keydown', {
                                key: 'Enter',
                                code: 'Enter',
                                keyCode: 13,
                                which: 13,
                                bubbles: true,
                                cancelable: true
                            });
                            el.dispatchEvent(enterDown);
                            
                            const enterPress = new KeyboardEvent('keypress', {
                                key: 'Enter',
                                code: 'Enter',
                                keyCode: 13,
                                which: 13,
                                bubbles: true,
                                cancelable: true
                            });
                            el.dispatchEvent(enterPress);
                            
                            const enterUp = new KeyboardEvent('keyup', {
                                key: 'Enter',
                                code: 'Enter',
                                keyCode: 13,
                                which: 13,
                                bubbles: true,
                                cancelable: true
                            });
                            el.dispatchEvent(enterUp);
                            
                            return { success: true, method: 'keyboard_events' };
                        })()
                    "#;
                    
                    page.evaluate(js).await
                        .map_err(|e| JobError::script_error(format!("PressKey (Enter) failed: {}", e)))?;
                } else {
                    // Generic key press for other keys
                    let js = format!(
                        r#"
                        (() => {{
                            const el = document.activeElement || document.body;
                            const keyDown = new KeyboardEvent('keydown', {{ key: {}, bubbles: true, cancelable: true }});
                            el.dispatchEvent(keyDown);
                            const keyPress = new KeyboardEvent('keypress', {{ key: {}, bubbles: true, cancelable: true }});
                            el.dispatchEvent(keyPress);
                            const keyUp = new KeyboardEvent('keyup', {{ key: {}, bubbles: true, cancelable: true }});
                            el.dispatchEvent(keyUp);
                            return {{ success: true }};
                        }})()
                        "#,
                        json!(key), json!(key), json!(key)
                    );
                    
                    page.evaluate(js).await
                        .map_err(|e| JobError::script_error(format!("PressKey failed: {}", e)))?;
                }
                
                sleep(Duration::from_millis(500)).await;
                output.insert("press_key".to_string(), json!(key));
                Ok(())
            }
            BrowserAction::Scroll { target } => {
                self.scroll(page, target).await?;
                output.insert("scroll".to_string(), json!(true));
                Ok(())
            }
            BrowserAction::Screenshot { path, full_page } => {
                let mut params = CaptureScreenshotParams::builder().format(CaptureScreenshotFormat::Png);
                if *full_page {
                    params = params.capture_beyond_viewport(true);
                }

                let bytes = page.screenshot(params.build()).await
                    .map_err(|e| JobError::browser_error(format!("Screenshot failed: {}", e)))?;

                tokio::fs::write(path, &bytes).await
                    .map_err(|e| JobError::browser_error(format!("Failed to save screenshot: {}", e)))?;
                
                output.insert("screenshot".to_string(), json!(path));
                Ok(())
            }
            BrowserAction::Hover { selector } => {
                self.wait_strategy.wait_for_element(page, selector, 10000, false).await?;
                
                let js = js::build_js_call(js::element::HOVER_ELEMENT, &[json!(selector)]);
                page.evaluate(js).await
                    .map_err(|e| JobError::script_error(format!("Hover failed: {}", e)))?;
                
                output.insert(format!("hover:{}", selector), json!(true));
                Ok(())
            }
            BrowserAction::Select { selector, value } => {
                self.wait_strategy.wait_for_element(page, selector, 10000, false).await?;
                
                let js = js::build_js_call(js::element::SELECT_OPTION, &[json!(selector), json!(value)]);
                page.evaluate(js).await
                    .map_err(|e| JobError::script_error(format!("Select failed: {}", e)))?;
                
                output.insert(format!("select:{}", selector), json!(value));
                Ok(())
            }
            BrowserAction::SetCookie { name, value, domain } => {
                let js = js::build_js_call(js::element::SET_COOKIE, &[json!(name), json!(value), json!(domain)]);
                page.evaluate(js).await
                    .map_err(|e| JobError::script_error(format!("SetCookie failed: {}", e)))?;
                
                output.insert(format!("set_cookie:{}", name), json!(value));
                Ok(())
            }
            BrowserAction::ExecuteScript { script } => {
                let result = page.evaluate(script.clone()).await
                    .map_err(|e| JobError::script_error(format!("ExecuteScript failed: {}", e)))?;
                
                output.insert("execute_script".to_string(), result.value().cloned().unwrap_or(json!(null)));
                Ok(())
            }
            BrowserAction::Navigate { url } => {
                page.goto(url).await
                    .map_err(|e| JobError::navigation_error(format!("Navigate failed: {}", e)))?;
                self.wait_strategy.wait_for_stable(page, 30000).await?;
                output.insert("navigate".to_string(), json!(url));
                Ok(())
            }
            BrowserAction::WaitForNavigation { timeout_ms } => {
                self.wait_strategy.wait_for_navigation(page, *timeout_ms).await?;
                output.insert("wait_for_navigation".to_string(), json!(true));
                Ok(())
            }
            BrowserAction::WaitFor { selector, timeout_ms } => {
                self.wait_strategy.wait_for_element(page, selector, *timeout_ms, false).await?;
                output.insert(format!("waitfor:{}", selector), json!(true));
                Ok(())
            }
            BrowserAction::WaitAndClick { selector, timeout_ms } => {
                self.wait_strategy.wait_for_element(page, selector, *timeout_ms, true).await?;
                self.scroll_to_element(page, selector).await?;
                
                let js = js::build_js_call(js::element::SAFE_CLICK, &[json!(selector)]);
                page.evaluate(js).await
                    .map_err(|e| JobError::script_error(format!("WaitAndClick failed: {}", e)))?;
                
                sleep(Duration::from_millis(300)).await;
                output.insert(format!("wait_and_click:{}", selector), json!(true));
                Ok(())
            }
            BrowserAction::HandleCookieBanner { timeout_ms } => {
                let patterns = js::cookie::COOKIE_PATTERNS;
                let js = js::build_js_call(js::cookie::FIND_AND_CLICK_COOKIE, &[json!(patterns)]);
                
                let start = std::time::Instant::now();
                let timeout = Duration::from_millis(*timeout_ms);
                
                while start.elapsed() < timeout {
                    if let Ok(result) = page.evaluate(js.clone()).await {
                        if let Some(val) = result.value() {
                            if let Some(obj) = val.as_object() {
                                if obj.get("clicked").and_then(|v| v.as_bool()) == Some(true) {
                                    let text = obj.get("text").and_then(|v| v.as_str()).unwrap_or("");
                                    output.insert("cookie_banner_handled".to_string(), 
                                        json!({ "clicked": true, "button_text": text }));
                                    sleep(Duration::from_millis(1000)).await; // Wait for banner to disappear
                                    return Ok(());
                                }
                            }
                        }
                    }
                    sleep(Duration::from_millis(500)).await;
                }
                
                output.insert("cookie_banner_handled".to_string(), 
                    json!({ "clicked": false, "reason": "not found" }));
                Ok(())
            }
        }
    }
}