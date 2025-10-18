use rocky_core::{Job, JobWorker};
use rocky_storage::Storage;
use futures::stream::{FuturesUnordered, StreamExt};
use std::sync::Arc;
use tokio::sync::{mpsc, Semaphore};

pub struct Scheduler<W: JobWorker + 'static, S: Storage + 'static> {
    worker: Arc<W>,
    storage: Arc<S>,
    sender: mpsc::Sender<Job>,
    concurrency_limit: Arc<Semaphore>,
}

impl<W: JobWorker + 'static, S: Storage + 'static> Clone for Scheduler<W, S> {
    fn clone(&self) -> Self {
        Self {
            worker: Arc::clone(&self.worker),
            storage: Arc::clone(&self.storage),
            sender: self.sender.clone(),
            concurrency_limit: Arc::clone(&self.concurrency_limit),
        }
    }
}

impl<W: JobWorker + 'static, S: Storage + 'static> Scheduler<W, S> {
    pub fn new(worker: W, storage: S, capacity: usize, max_concurrent: usize) -> (Self, mpsc::Receiver<Job>) {
        let (tx, rx) = mpsc::channel(capacity);
        let scheduler = Self {
            worker: Arc::new(worker),
            storage: Arc::new(storage),
            sender: tx,
            concurrency_limit: Arc::new(Semaphore::new(max_concurrent)),
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
                    let permit = Arc::clone(&self.concurrency_limit).acquire_owned().await.unwrap();

                    futures.push(async move {
                        // permit ensures only max_concurrent jobs run at a time
                        let result = worker.execute(&job).await;
                        if let Ok(ref r) = result {
                            let _ = storage.save_result(r).await;
                        }
                        drop(permit); // release the semaphore
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
                    break; // no more jobs and all futures finished
                }
            }
        }
    }
}