use parser::ParserWorker;
use rocky_core::{Action, Job, JobWorker};
use tokio;

#[tokio::main]
async fn main() {
    let job = Job {
        id: "job-001".to_string(),
        url: "https://example.com".to_string(),
        use_browser: false,
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
    };

    let worker = ParserWorker::new();

    match worker.execute(&job).await {
        Ok(result) => {
            println!("Job {} succeeded!", result.job_id);
            println!(
                "Output:\n{}",
                serde_json::to_string_pretty(&result.output).unwrap()
            );
        }
        Err(err) => {
            eprintln!("Job {} failed: {}", job.id, err);
        }
    }
}
