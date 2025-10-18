use rocky_core::{Action, Job};
use rocky_parser::ParserWorker;
use rocky_scheduler::Scheduler;
use rocky_storage::JsonFileStorage;

#[tokio::main]
async fn main() {
    let worker = ParserWorker::new();
    let storage = JsonFileStorage::new("results");
    let scheduler = Scheduler::new(worker, storage);

    let jobs = vec![
        Job {
            id: "job-001".to_string(),
            url: "https://example.com".to_string(),
            use_browser: false,
            actions: vec![
                Action::WaitFor { selector: "h1".to_string(), timeout_ms: 5000 },
                Action::Extract { selector: "p".to_string(), attr: None },
            ],
        },
        Job {
            id: "job-002".to_string(),
            url: "https://example.org".to_string(),
            use_browser: false,
            actions: vec![
                Action::WaitFor { selector: "h1".to_string(), timeout_ms: 5000 },
            ],
        },
    ];

    scheduler.run_jobs(jobs).await;
}