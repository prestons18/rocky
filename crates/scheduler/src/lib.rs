use futures::stream::{FuturesUnordered, StreamExt};
use rocky_core::{Job, JobWorker};
use rocky_storage::Storage;
use std::sync::Arc;

pub struct Scheduler<W: JobWorker + 'static, S: Storage + 'static> {
    worker: Arc<W>,
    storage: Arc<S>,
}

impl<W: JobWorker + 'static, S: Storage + 'static> Scheduler<W, S> {
    pub fn new(worker: W, storage: S) -> Self {
        Self {
            worker: Arc::new(worker),
            storage: Arc::new(storage),
        }
    }

    pub async fn run_jobs(&self, jobs: Vec<Job>) {
        let mut futures = FuturesUnordered::new();

        for job in jobs {
            let worker = Arc::clone(&self.worker);
            let storage = Arc::clone(&self.storage);
            futures.push(async move {
                let result = worker.execute(&job).await;
                if let Ok(ref r) = result {
                    let _ = storage.save_result(r).await;
                }
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
                Err(err) => eprintln!("Job {} failed: {}", job_id, err),
            }
        }
    }
}