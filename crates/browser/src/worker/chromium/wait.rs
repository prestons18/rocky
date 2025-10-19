use chromiumoxide::page::Page;
use rocky_core::JobError;
use serde_json::json;
use std::time::{Duration, Instant};
use tokio::time::sleep;
use crate::shared::{js, to_job_error, TimeoutConfig};

pub struct WaitStrategy {
    config: TimeoutConfig,
}

impl WaitStrategy {
    pub fn new(config: TimeoutConfig) -> Self {
        Self { config }
    }

    pub async fn wait_for_element(
        &self,
        page: &Page,
        selector: &str,
        timeout_ms: u64,
        check_clickable: bool,
    ) -> Result<(), JobError> {
        let timeout = std::time::Duration::from_millis(timeout_ms);
        let start = Instant::now();
        let selector_json = json!(selector);
        let mut last_state = String::new();
        
        loop {
            let js = js::build_js_call(js::element::CHECK_ELEMENT_STATE, &[selector_json.clone()]);
            
            // Handle potential context loss gracefully
            let result = match page.evaluate(js).await {
                Ok(r) => r,
                Err(e) => {
                    let err_str = e.to_string();
                    if err_str.contains("Cannot find context") || err_str.contains("Execution context was destroyed") {
                        // Page is navigating, wait a bit and retry
                        sleep(Duration::from_millis(500)).await;
                        continue;
                    }
                    return Err(to_job_error(e, "WaitFor"));
                }
            };
            
            if let Some(state) = result.value() {
                if let Some(obj) = state.as_object() {
                    let exists = obj.get("exists").and_then(|v| v.as_bool()).unwrap_or(false);
                    let visible = obj.get("visible").and_then(|v| v.as_bool()).unwrap_or(false);
                    let obscured = obj.get("obscured").and_then(|v| v.as_bool()).unwrap_or(false);
                    let disabled = obj.get("disabled").and_then(|v| v.as_bool()).unwrap_or(false);
                    
                    let current_state = format!("exists:{} visible:{} obscured:{} disabled:{}", 
                        exists, visible, obscured, disabled);
                    
                    if current_state != last_state {
                        println!("    Element '{}' state: {}", selector, current_state);
                        last_state = current_state;
                    }
                    
                    if !exists {
                        if start.elapsed() > timeout {
                            return Err(JobError::element_not_found(
                                format!("Element '{}' not found after {}ms", selector, timeout_ms)
                            ).with_context(json!({ "selector": selector, "timeout_ms": timeout_ms })));
                        }
                    } else if !visible {
                        if start.elapsed() > timeout {
                            return Err(JobError::element_not_found(
                                format!("Element '{}' exists but not visible", selector)
                            ).with_context(json!({ "selector": selector, "hint": "Element may be hidden with CSS" })));
                        }
                    } else if obscured {
                        let obscured_by = obj.get("obscuredBy").and_then(|v| v.as_str()).unwrap_or("unknown");
                        if start.elapsed() > timeout {
                            return Err(JobError::element_not_found(
                                format!("Element '{}' obscured by {}", selector, obscured_by)
                            ).with_context(json!({ 
                                "selector": selector,
                                "obscured_by": obscured_by,
                                "suggestion": "Use HandleCookieBanner or WaitAndClick" 
                            })));
                        }
                    } else if check_clickable && disabled {
                        if start.elapsed() > timeout {
                            return Err(JobError::element_not_found(
                                format!("Element '{}' is disabled", selector)
                            ));
                        }
                    } else {
                        println!("    ✓ Element '{}' ready", selector);
                        return Ok(());
                    }
                }
            }
            
            if start.elapsed() > timeout {
                return Err(JobError::timeout_error(
                    format!("Timeout waiting for element '{}'", selector)
                ).with_context(json!({ "selector": selector, "timeout_ms": timeout_ms })));
            }
            
            sleep(self.config.check_interval).await;
        }
    }
    
    pub async fn wait_for_stable(&self, page: &Page, timeout_ms: u64) -> Result<(), JobError> {
        let timeout = std::time::Duration::from_millis(timeout_ms);
        let start = Instant::now();
        let mut stable_checks = 0;
        let required_stable_checks = 5;
        
        println!("    Waiting for page to stabilize...");
        
        // First, wait a bit for the navigation to start
        sleep(Duration::from_millis(500)).await;
        
        loop {
            let js = js::build_js_call(js::wait::CHECK_LOADING, &[]);
            
            // Handle context loss gracefully during navigation
            let result = match page.evaluate(js).await {
                Ok(r) => r,
                Err(e) => {
                    let err_str = e.to_string();
                    if err_str.contains("Cannot find context") || err_str.contains("Execution context was destroyed") {
                        println!("    Page context changed (navigating), waiting...");
                        stable_checks = 0;
                        sleep(Duration::from_millis(1000)).await;
                        continue;
                    }
                    return Err(to_job_error(e, "WaitForStable"));
                }
            };
            
            if let Some(state) = result.value() {
                if let Some(obj) = state.as_object() {
                    let ready = obj.get("readyState").and_then(|v| v.as_str()) == Some("complete");
                    let active = obj.get("activeRequests").and_then(|v| v.as_u64()).unwrap_or(0);
                    
                    if ready && active == 0 {
                        stable_checks += 1;
                        if stable_checks >= required_stable_checks {
                            println!("    ✓ Page stabilized ({}ms)", start.elapsed().as_millis());
                            sleep(self.config.settle_delay).await;
                            return Ok(());
                        }
                    } else {
                        if stable_checks > 0 {
                            println!("    Page activity detected (ready:{} active:{}), resetting...", ready, active);
                        }
                        stable_checks = 0;
                    }
                }
            }
            
            if start.elapsed() > timeout {
                println!("    ⚠ Page stabilization timeout, continuing anyway...");
                return Ok(()); // Don't fail, just continue
            }
            
            sleep(self.config.check_interval).await;
        }
    }
    
    pub async fn wait_for_navigation(&self, page: &Page, timeout_ms: u64) -> Result<(), JobError> {
        println!("    Waiting for navigation...");
        
        // Wait a moment for navigation to actually start
        sleep(Duration::from_millis(1000)).await;
        
        // Now wait for it to complete
        self.wait_for_stable(page, timeout_ms).await
    }
}