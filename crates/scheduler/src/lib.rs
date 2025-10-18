use futures::stream::{FuturesUnordered, StreamExt};
use rocky_core::{Job, JobWorker};
use std::sync::Arc;

pub struct Scheduler<W: JobWorker + 'static> {
    worker: Arc<W>,
}

impl<W: JobWorker + 'static> Scheduler<W> {
    pub fn new(worker: W) -> Self {
        Self {
            worker: Arc::new(worker),
        }
    }

    pub async fn run_jobs(&self, jobs: Vec<Job>) {
        let mut futures = FuturesUnordered::new();

        for job in jobs {
            let worker = Arc::clone(&self.worker);
            futures.push(async move {
                let result = worker.execute(&job).await;
                (job.id.clone(), result)
            });
        }

        while let Some((job_id, res)) = futures.next().await {
            match res {
                Ok(result) => {
                    println!("Job {} succeeded", job_id);
                    println!(
                        "Output: {}",
                        serde_json::to_string_pretty(&result.output).unwrap()
                    );
                }
                Err(err) => {
                    eprintln!("Job {} failed: {}", job_id, err);
                }
            }
        }
    }
}
