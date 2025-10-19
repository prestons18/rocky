use browser::BrowserWorker;
use rocky_core::{
    Action, BrowserAction, BrowserConfig, BrowserType, Job, ScrapingAction, ScrollTarget,
};
use rocky_parser::ParserWorker;
use rocky_scheduler::Scheduler;
use rocky_storage::JsonFileStorage;
use tokio::time::{Duration, sleep};

#[tokio::main]
async fn main() {
    let parser = ParserWorker::new();
    let browser = BrowserWorker::new();
    let storage = JsonFileStorage::new("results");

    let (scheduler, receiver) = Scheduler::new(parser, browser, storage, 20, 4);
    let scheduler_handle = scheduler.clone();
    tokio::spawn(async move { scheduler.run(receiver).await });

    let jobs = vec![
        // Simple scraping job with parser
        Job {
            id: "job-001".to_string(),
            url: "https://example.com".to_string(),
            use_browser: false,
            actions: vec![
                Action::Scraping(ScrapingAction::Extract {
                    selector: "p".to_string(),
                    attr: None,
                }),
                Action::Scraping(ScrapingAction::ExtractMultiple {
                    selector: "a".to_string(),
                    attrs: vec!["href".to_string(), "text".to_string()],
                }),
            ],
            browser_config: None,
        },
        // Browser automation job with interactions
        Job {
            id: "job-002".to_string(),
            url: "https://example.org".to_string(),
            use_browser: true,
            actions: vec![
                // Handle any potential cookie banners
                Action::Browser(BrowserAction::HandleCookieBanner { timeout_ms: 2000 }),
                // Wait for main heading to be visible
                Action::Scraping(ScrapingAction::WaitFor {
                    selector: "h1".to_string(),
                    timeout_ms: 5000,
                }),
                Action::Browser(BrowserAction::Scroll {
                    target: ScrollTarget::Bottom,
                }),
                Action::Scraping(ScrapingAction::Extract {
                    selector: "p".to_string(),
                    attr: None,
                }),
                Action::Browser(BrowserAction::Screenshot {
                    path: "results/job-002-screenshot.png".to_string(),
                    full_page: true,
                }),
            ],
            browser_config: Some(BrowserConfig {
                browser_type: BrowserType::Chromium,
                headless: true,
                viewport_width: Some(1920),
                viewport_height: Some(1080),
                fail_on_captcha: true,
            }),
        },
        Job {
            id: "job-003".to_string(),
            url: "https://www.google.com".to_string(),
            use_browser: true,
            actions: vec![
                // First, handle any cookie banners
                Action::Browser(BrowserAction::HandleCookieBanner { timeout_ms: 3000 }),
                // Wait for search box to be clickable and click it (focuses the input)
                Action::Browser(BrowserAction::WaitAndClick {
                    selector: "textarea[name='q']".to_string(),
                    timeout_ms: 5000,
                }),
                // Type the search query
                Action::Browser(BrowserAction::Type {
                    selector: "textarea[name='q']".to_string(),
                    text: "Rust programming language".to_string(),
                    clear_first: true,
                }),
                // Submit the search
                Action::Browser(BrowserAction::PressKey {
                    key: "Enter".to_string(),
                }),
                // Wait for navigation to results page
                Action::Browser(BrowserAction::WaitForNavigation { timeout_ms: 10000 }),
                // Verify we're on the search results page by checking for results container
                Action::Scraping(ScrapingAction::WaitFor {
                    selector: "#search".to_string(),
                    timeout_ms: 5000,
                }),
                // Extract all h3 elements (search result titles)
                Action::Scraping(ScrapingAction::Extract {
                    selector: "h3".to_string(),
                    attr: None,
                }),
                // Also extract h3s with their parent link URLs
                Action::Scraping(ScrapingAction::ExtractMultiple {
                    selector: "h3".to_string(),
                    attrs: vec!["text".to_string()],
                }),
                // Scroll to see more results
                Action::Browser(BrowserAction::Scroll {
                    target: ScrollTarget::Bottom,
                }),
                // Take a screenshot of the results
                Action::Browser(BrowserAction::Screenshot {
                    path: "results/job-003-screenshot.png".to_string(),
                    full_page: false,
                }),
            ],
            browser_config: Some(BrowserConfig {
                browser_type: BrowserType::Chromium,
                headless: false,
                viewport_width: Some(1280),
                viewport_height: Some(720),
                fail_on_captcha: true,
            }),
        },
    ];

    for job in jobs {
        scheduler_handle.submit(job).unwrap();
        sleep(Duration::from_millis(200)).await;
    }

    sleep(Duration::from_secs(10)).await;
}
