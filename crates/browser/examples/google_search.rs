use browser::{BrowserWorker, TimeoutConfig};
use rocky_core::{Job, Action, BrowserAction, ScrapingAction, JobWorker, BrowserConfig, BrowserType};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = TimeoutConfig::patient();
    let worker = BrowserWorker::with_config(config);
    
    let job = Job {
        id: "search-1".to_string(),
        url: "https://google.com".to_string(),
        use_browser: true,
        actions: vec![
            Action::Browser(BrowserAction::HandleCookieBanner { timeout_ms: 5000 }),
            
            // Type in search box
            Action::Browser(BrowserAction::Type {
                selector: "textarea[name='q'], input[name='q']".to_string(),
                text: "rust web scraping".to_string(),
                clear_first: true,
            }),
            
            // Press Enter
            Action::Browser(BrowserAction::PressKey { key: "Enter".to_string() }),
            
            // Wait for navigation
            Action::Browser(BrowserAction::WaitForNavigation { timeout_ms: 30000 }),
            
            // Take screenshot first to see what we got
            Action::Browser(BrowserAction::Screenshot {
                path: "/tmp/google_search_after_nav.png".to_string(),
                full_page: true,
            }),
            
            // Wait for search results container
            Action::Browser(BrowserAction::WaitFor {
                selector: "#search".to_string(),
                timeout_ms: 20000,
            }),
            
            // Extract all text from search results
            Action::Scraping(ScrapingAction::Extract {
                selector: "#search h3".to_string(),
                attr: None,
            }),
            
            Action::Browser(BrowserAction::Screenshot {
                path: "/tmp/google_search_results.png".to_string(),
                full_page: true,
            }),
        ],
        browser_config: Some(BrowserConfig {
            browser_type: BrowserType::Chromium,
            headless: false,
            viewport_width: Some(1920),
            viewport_height: Some(1080),
            fail_on_captcha: true, // Enable CAPTCHA detection
        }),
    };
    
    println!("🔍 Starting Google search...\n");
    let result = match worker.execute(&job).await {
        Ok(r) => r,
        Err(e) => {
            eprintln!("{}", e);
            return Err(e.into());
        }
    };
    
    println!("\n=== Search Results ===");
    
    for (key, value) in result.output.as_object().unwrap() {
        if key.starts_with("extract") {
            if let Some(arr) = value.as_array() {
                let count = arr.len();
                if count > 0 {
                    println!("\n📊 {}: {} items", key, count);
                    for (i, item) in arr.iter().take(5).enumerate() {
                        match item {
                            serde_json::Value::String(s) => {
                                if !s.trim().is_empty() {
                                    println!("  {}. {}", i + 1, s.chars().take(100).collect::<String>());
                                }
                            }
                            serde_json::Value::Object(obj) => {
                                if let Some(text) = obj.get("text") {
                                    if let Some(s) = text.as_str() {
                                        if !s.trim().is_empty() {
                                            println!("  {}. {}", i + 1, s.chars().take(100).collect::<String>());
                                        }
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
    }
    
    println!("\n📸 Screenshot: /tmp/google_search_results.png");
    println!("\n✨ Done!");
    
    Ok(())
}