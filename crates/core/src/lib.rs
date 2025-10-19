use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// Actions for basic scraping (HTTP-only, no JavaScript)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ScrapingAction {
    Fetch {
        url: String,
    },
    Extract {
        selector: String,
        attr: Option<String>,
    },
    ExtractMultiple {
        selector: String,
        attrs: Vec<String>,
    },
    WaitFor {
        selector: String,
        timeout_ms: u64,
    },
}

/// Actions that only work with browser workers (require JavaScript execution)
#[derive(Debug, Clone, Serialize, Deserialize)]
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
    WaitFor {
        selector: String,
        timeout_ms: u64,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ScrollTarget {
    Element { selector: String },
    Position { x: i32, y: i32 },
    Bottom,
    Top,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
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

/// Error categories for better error handling and recovery
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ErrorCategory {
    /// Network-related errors (timeouts, connection failures)
    Network,
    /// Element not found or selector issues
    ElementNotFound,
    /// JavaScript execution errors
    ScriptExecution,
    /// Navigation or page load errors
    Navigation,
    /// Browser/driver errors
    Browser,
    /// Parsing errors (HTML/JSON)
    Parsing,
    /// Timeout errors
    Timeout,
    /// Authentication/authorization errors
    Auth,
    /// Rate limiting or blocking
    RateLimit,
    /// Unknown or uncategorized errors
    Unknown,
}

/// Structured error with context for better debugging and recovery
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobError {
    /// Error category for programmatic handling
    pub category: ErrorCategory,
    /// Human-readable error message
    pub message: String,
    /// Optional context (URL, selector, action type, etc.)
    pub context: serde_json::Value,
    /// Whether this error is potentially recoverable
    pub recoverable: bool,
    /// Suggested retry delay in milliseconds
    pub retry_after_ms: Option<u64>,
}

impl JobError {
    pub fn new(category: ErrorCategory, message: impl Into<String>) -> Self {
        Self {
            category,
            message: message.into(),
            context: serde_json::json!({}),
            recoverable: false,
            retry_after_ms: None,
        }
    }

    pub fn with_context(mut self, context: serde_json::Value) -> Self {
        self.context = context;
        self
    }

    pub fn recoverable(mut self) -> Self {
        self.recoverable = true;
        self
    }

    pub fn with_retry_delay(mut self, ms: u64) -> Self {
        self.retry_after_ms = Some(ms);
        self.recoverable = true;
        self
    }

    // Convenience constructors
    pub fn fetch_error(message: impl Into<String>) -> Self {
        Self::new(ErrorCategory::Network, message).recoverable().with_retry_delay(1000)
    }

    pub fn element_not_found(selector: impl Into<String>) -> Self {
        let selector = selector.into();
        Self::new(ErrorCategory::ElementNotFound, format!("Element not found: {}", selector))
            .with_context(serde_json::json!({ "selector": selector }))
    }

    pub fn timeout_error(message: impl Into<String>) -> Self {
        Self::new(ErrorCategory::Timeout, message).recoverable().with_retry_delay(2000)
    }

    pub fn script_error(message: impl Into<String>) -> Self {
        Self::new(ErrorCategory::ScriptExecution, message)
    }

    pub fn navigation_error(message: impl Into<String>) -> Self {
        Self::new(ErrorCategory::Navigation, message).recoverable().with_retry_delay(1500)
    }

    pub fn browser_error(message: impl Into<String>) -> Self {
        Self::new(ErrorCategory::Browser, message)
    }

    pub fn parsing_error(message: impl Into<String>) -> Self {
        Self::new(ErrorCategory::Parsing, message)
    }
}

impl std::fmt::Display for JobError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{:?}] {}", self.category, self.message)
    }
}

impl std::error::Error for JobError {}

/// Context passed to error healing hooks
#[derive(Debug, Clone)]
pub struct ErrorContext {
    pub job_id: String,
    pub error: JobError,
    pub attempt: u32,
    pub max_attempts: u32,
}

/// Result of an error healing attempt
#[derive(Debug, Clone)]
pub enum HealingAction {
    /// Retry the job immediately
    Retry,
    /// Retry after a delay (milliseconds)
    RetryAfter(u64),
    /// Skip this job and mark as failed
    Skip,
    /// Abort the entire workflow
    Abort,
}

/// Trait for implementing custom error healing logic
#[async_trait]
pub trait ErrorHealer: Send + Sync {
    /// Called when a job encounters an error
    /// Returns the action to take (retry, skip, abort)
    async fn heal(&self, context: &ErrorContext) -> HealingAction;
}

/// Default error healer with simple retry logic
pub struct DefaultErrorHealer {
    pub max_retries: u32,
}

impl DefaultErrorHealer {
    pub fn new(max_retries: u32) -> Self {
        Self { max_retries }
    }
}

#[async_trait]
impl ErrorHealer for DefaultErrorHealer {
    async fn heal(&self, context: &ErrorContext) -> HealingAction {
        if context.attempt >= self.max_retries {
            return HealingAction::Skip;
        }

        if !context.error.recoverable {
            return HealingAction::Skip;
        }

        match context.error.retry_after_ms {
            Some(delay) => HealingAction::RetryAfter(delay),
            None => HealingAction::Retry,
        }
    }
}

#[async_trait]
pub trait JobWorker: Send + Sync {
    async fn execute(&self, job: &Job) -> Result<JobResult, JobError>;
}
