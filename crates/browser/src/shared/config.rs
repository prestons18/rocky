use std::time::Duration;

#[derive(Debug, Clone)]
pub struct TimeoutConfig {
    pub element_wait: Duration,
    pub navigation: Duration,
    pub page_stable: Duration,
    pub cookie_banner: Duration,
    pub check_interval: Duration,
    pub settle_delay: Duration,
}

impl Default for TimeoutConfig {
    fn default() -> Self {
        Self {
            element_wait: Duration::from_millis(15000),
            navigation: Duration::from_millis(30000),
            page_stable: Duration::from_millis(30000),
            cookie_banner: Duration::from_millis(5000),
            check_interval: Duration::from_millis(300), // Increased from 200ms
            settle_delay: Duration::from_millis(1000),  // Increased from 500ms
        }
    }
}

impl TimeoutConfig {
    pub fn with_element_wait(mut self, ms: u64) -> Self {
        self.element_wait = Duration::from_millis(ms);
        self
    }

    pub fn with_navigation(mut self, ms: u64) -> Self {
        self.navigation = Duration::from_millis(ms);
        self
    }

    pub fn fast() -> Self {
        Self {
            element_wait: Duration::from_millis(8000),
            navigation: Duration::from_millis(20000),
            page_stable: Duration::from_millis(20000),
            cookie_banner: Duration::from_millis(3000),
            check_interval: Duration::from_millis(200),
            settle_delay: Duration::from_millis(500),
        }
    }
    
    pub fn patient() -> Self {
        Self {
            element_wait: Duration::from_millis(30000),
            navigation: Duration::from_millis(60000),
            page_stable: Duration::from_millis(60000),
            cookie_banner: Duration::from_millis(10000),
            check_interval: Duration::from_millis(500),
            settle_delay: Duration::from_millis(2000),
        }
    }
}