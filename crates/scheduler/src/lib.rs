use rocky_core::{Job, JobWorker};
use rocky_storage::Storage;
use futures::stream::{FuturesUnordered, StreamExt};
use std::sync::Arc;
use tokio::sync::{mpsc, Semaphore};

pub struct Scheduler<S: Storage + 'static> {
    parser_worker: Arc<dyn JobWorker>,
    browser_worker: Arc<dyn JobWorker>,
    storage: Arc<S>,
    sender: mpsc::Sender<Job>,
    concurrency_limit: Arc<Semaphore>,
}

impl<S: Storage + 'static> Clone for Scheduler<S> {
    fn clone(&self) -> Self {
        Self {
            parser_worker: Arc::clone(&self.parser_worker),
            browser_worker: Arc::clone(&self.browser_worker),
            storage: Arc::clone(&self.storage),
            sender: self.sender.clone(),
            concurrency_limit: Arc::clone(&self.concurrency_limit),
        }
    }
}

impl<S: Storage + 'static> Scheduler<S> {
    pub fn new<P: JobWorker + 'static, B: JobWorker + 'static>(
        parser: P, 
        browser: B, 
        storage: S, 
        capacity: usize, 
        max_concurrent: usize
    ) -> (Self, mpsc::Receiver<Job>) {
        let (tx, rx) = mpsc::channel(capacity);
        let scheduler = Self {
            parser_worker: Arc::new(parser),
            browser_worker: Arc::new(browser),
            storage: Arc::new(storage),
            sender: tx,
            concurrency_limit: Arc::new(Semaphore::new(max_concurrent)),
        };
        (scheduler, rx)
    }

    pub fn with_single_worker<W: JobWorker + 'static>(
        worker: W,
        storage: S,
        capacity: usize,
        max_concurrent: usize
    ) -> (Self, mpsc::Receiver<Job>) {
        let worker: Arc<dyn JobWorker> = Arc::new(worker);
        let (tx, rx) = mpsc::channel(capacity);
        let scheduler = Self {
            parser_worker: Arc::clone(&worker),
            browser_worker: worker,
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
                    let storage = Arc::clone(&self.storage);
                    let permit = Arc::clone(&self.concurrency_limit).acquire_owned().await.unwrap();

                    let worker = if job.use_browser {
                        Arc::clone(&self.browser_worker)
                    } else {
                        Arc::clone(&self.parser_worker)
                    };

                    futures.push(async move {
                        let result = worker.execute(&job).await;
                        if let Ok(ref r) = result {
                            let _ = storage.save_result(r).await;
                        }
                        drop(permit);
                        (job.id.clone(), result)
                    });
                }
                Some((job_id, res)) = futures.next() => {
                    match res {
                        Ok(_result) => println!("Job {} succeeded", job_id),
                        Err(err) => eprintln!("Job {} failed: {}", job_id, err),
                    }
                }
                else => break,
            }
        }
    }
}