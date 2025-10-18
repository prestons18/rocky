use browser::BrowserWorker;
use rocky_core::{Action, BrowserConfig, BrowserType, Job};
use rocky_parser::ParserWorker;
use rocky_scheduler::Scheduler;
use rocky_storage::JsonFileStorage;
use tokio::time::{Duration, sleep};

#[tokio::main]
async fn main() {
    let parser = ParserWorker::new();
    let browser = BrowserWorker::new().await;
    let storage = JsonFileStorage::new("results");

    let (scheduler, receiver) = Scheduler::new(parser, browser, storage, 20, 4);
    let scheduler_handle = scheduler.clone();
    tokio::spawn(async move { scheduler.run(receiver).await });

    let jobs = vec![
        Job {
            id: "job-001".to_string(),
            url: "https://example.com".to_string(),
            use_browser: false,
            actions: vec![Action::Extract {
                selector: "p".to_string(),
                attr: None,
            }],
            browser_config: None,
        },
        Job {
            id: "job-002".to_string(),
            url: "https://example.org".to_string(),
            use_browser: true,
            actions: vec![
                Action::WaitFor {
                    selector: "h1".to_string(),
                    timeout_ms: 5000,
                },
                Action::Extract {
                    selector: "p".to_string(),
                    attr: None,
                },
            ],
            browser_config: Some(BrowserConfig {
                browser_type: BrowserType::Chromium,
                headless: true,
                viewport_width: Some(1920),
                viewport_height: Some(1080),
            }),
        },
    ];

    for job in jobs {
        scheduler_handle.submit(job).unwrap();
        sleep(Duration::from_millis(200)).await;
    }

    sleep(Duration::from_secs(5)).await;
}
