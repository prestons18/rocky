use rocky_core::{Action, Job};
use rocky_parser::ParserWorker;
use rocky_scheduler::Scheduler;
use rocky_storage::JsonFileStorage;
use tokio::time::{sleep, Duration};

#[tokio::main]
async fn main() {
    let worker = ParserWorker::new();
    let storage = JsonFileStorage::new("results");
    let (scheduler, receiver) = Scheduler::new(worker, storage, 20, 4); // max 4 concurrent jobs

    let scheduler_clone = scheduler.clone();
    tokio::spawn(async move {
        scheduler_clone.run(receiver).await;
    });

    for i in 1..=10 {
        let job = Job {
            id: format!("job-00{}", i),
            url: "https://example.com".to_string(),
            use_browser: false,
            actions: vec![
                Action::WaitFor { selector: "h1".to_string(), timeout_ms: 5000 },
                Action::Extract { selector: "p".to_string(), attr: None },
            ],
        };
        scheduler.submit(job).unwrap();
        sleep(Duration::from_millis(200)).await;
    }

    sleep(Duration::from_secs(15)).await;
}