use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Actions that work with both browser and parser workers
#[derive(Debug, Serialize, Deserialize)]
pub enum ScrapingAction {
    WaitFor {
        selector: String,
        timeout_ms: u64,
    },
    Extract {
        selector: String,
        attr: Option<String>,
    },
    ExtractMultiple {
        selector: String,
        attrs: Vec<String>,
    },
}

#[derive(Debug, Serialize, Deserialize)]
pub enum BrowserAction {
    /// Click an element
    Click {
        selector: String,
        timeout_ms: u64,
    },
    Type {
        selector: String,
        text: String,
        clear_first: bool,
    },
    PressKey {
        key: String,
    },
    Scroll {
        target: ScrollTarget,
    },
    Screenshot {
        path: String,
        full_page: bool,
    },
    Hover {
        selector: String,
    },
    Select {
        selector: String,
        value: String,
    },
    Navigate {
        url: String,
    },
    ExecuteScript {
        script: String,
    },
    SetCookie {
        name: String,
        value: String,
        domain: Option<String>,
    },
    WaitForNavigation {
        timeout_ms: u64,
    },
}

#[derive(Debug, Serialize, Deserialize)]
pub enum ScrollTarget {
    Element { selector: String },
    Position { x: i32, y: i32 },
    Bottom,
    Top,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Action {
    Scraping(ScrapingAction),
    Browser(BrowserAction),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BrowserConfig {
    pub browser_type: BrowserType,
    pub headless: bool,
    pub viewport_width: Option<u32>,
    pub viewport_height: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum BrowserType {
    Chromium,
    Firefox,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Job {
    pub id: String,
    pub url: String,
    pub use_browser: bool,
    pub actions: Vec<Action>,
    pub browser_config: Option<BrowserConfig>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct JobResult {
    pub job_id: String,
    pub success: bool,
    pub output: serde_json::Value,
}

#[derive(Debug, Error)]
pub enum JobError {
    #[error("Failed to fetch page: {0}")]
    FetchError(String),

    #[error("Action failed: {0}")]
    ActionError(String),
}

#[async_trait]
pub trait JobWorker: Send + Sync {
    async fn execute(&self, job: &Job) -> Result<JobResult, JobError>;
}
