use rocky_core::{Job, JobWorker};
use futures::stream::{FuturesUnordered, StreamExt};
use std::sync::Arc;
use tokio::sync::mpsc;
use rocky_storage::Storage;

pub struct Scheduler<W: JobWorker + 'static, S: Storage + 'static> {
    worker: Arc<W>,
    storage: Arc<S>,
    sender: mpsc::Sender<Job>,
}

impl<W: JobWorker + 'static, S: Storage + 'static> Clone for Scheduler<W, S> {
    fn clone(&self) -> Self {
        Self {
            worker: Arc::clone(&self.worker),
            storage: Arc::clone(&self.storage),
            sender: self.sender.clone(),
        }
    }
}

impl<W: JobWorker + 'static, S: Storage + 'static> Scheduler<W, S> {
    pub fn new(worker: W, storage: S, capacity: usize) -> (Self, mpsc::Receiver<Job>) {
        let (tx, rx) = mpsc::channel(capacity);
        let scheduler = Self {
            worker: Arc::new(worker),
            storage: Arc::new(storage),
            sender: tx,
        };
        (scheduler, rx)
    }

    pub fn submit(&self, job: Job) -> Result<(), mpsc::error::TrySendError<Job>> {
        self.sender.try_send(job)
    }

    pub async fn run(&self, mut receiver: mpsc::Receiver<Job>) {
        let mut futures = FuturesUnordered::new();

        loop {
            tokio::select! {
                Some(job) = receiver.recv() => {
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
                Some((job_id, res)) = futures.next() => {
                    match res {
                        Ok(_result) => println!("Job {} succeeded", job_id),
                        Err(err) => eprintln!("Job {} failed: {}", job_id, err),
                    }
                }
                else => {
                    // Both channel closed and no pending futures
                    break;
                }
            }
        }
    }
}