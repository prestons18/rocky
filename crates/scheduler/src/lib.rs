use rocky_core::{Job, JobWorker, ErrorHealer, ErrorContext, HealingAction, DefaultErrorHealer};
use rocky_storage::Storage;
use futures::stream::{FuturesUnordered, StreamExt};
use std::sync::Arc;
use std::collections::HashMap;
use tokio::sync::{mpsc, Semaphore, Mutex};

pub struct Scheduler<S: Storage + 'static> {
    parser_worker: Arc<dyn JobWorker>,
    browser_worker: Arc<dyn JobWorker>,
    storage: Arc<S>,
    sender: mpsc::Sender<Job>,
    concurrency_limit: Arc<Semaphore>,
    error_healer: Arc<dyn ErrorHealer>,
    retry_counts: Arc<Mutex<HashMap<String, u32>>>,
    max_retries: u32,
}

impl<S: Storage + 'static> Clone for Scheduler<S> {
    fn clone(&self) -> Self {
        Self {
            parser_worker: Arc::clone(&self.parser_worker),
            browser_worker: Arc::clone(&self.browser_worker),
            storage: Arc::clone(&self.storage),
            sender: self.sender.clone(),
            concurrency_limit: Arc::clone(&self.concurrency_limit),
            error_healer: Arc::clone(&self.error_healer),
            retry_counts: Arc::clone(&self.retry_counts),
            max_retries: self.max_retries,
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
        Self::with_healer(parser, browser, storage, capacity, max_concurrent, Arc::new(DefaultErrorHealer::new(3)))
    }

    pub fn with_healer<P: JobWorker + 'static, B: JobWorker + 'static, H: ErrorHealer + 'static>(
        parser: P, 
        browser: B, 
        storage: S, 
        capacity: usize, 
        max_concurrent: usize,
        healer: Arc<H>
    ) -> (Self, mpsc::Receiver<Job>) {
        let (tx, rx) = mpsc::channel(capacity);
        let scheduler = Self {
            parser_worker: Arc::new(parser),
            browser_worker: Arc::new(browser),
            storage: Arc::new(storage),
            sender: tx,
            concurrency_limit: Arc::new(Semaphore::new(max_concurrent)),
            error_healer: healer,
            retry_counts: Arc::new(Mutex::new(HashMap::new())),
            max_retries: 3,
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
            error_healer: Arc::new(DefaultErrorHealer::new(3)),
            retry_counts: Arc::new(Mutex::new(HashMap::new())),
            max_retries: 3,
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
                    let error_healer = Arc::clone(&self.error_healer);
                    let retry_counts = Arc::clone(&self.retry_counts);
                    let max_retries = self.max_retries;
                    let sender = self.sender.clone();

                    let worker = if job.use_browser {
                        Arc::clone(&self.browser_worker)
                    } else {
                        Arc::clone(&self.parser_worker)
                    };

                    futures.push(async move {
                        let result = worker.execute(&job).await;
                        
                        match result {
                            Ok(ref r) => {
                                let _ = storage.save_result(r).await;
                                // Clear retry count on success
                                retry_counts.lock().await.remove(&job.id);
                            }
                            Err(ref err) => {
                                // Get current retry count
                                let mut counts = retry_counts.lock().await;
                                let attempt = *counts.get(&job.id).unwrap_or(&0) + 1;
                                counts.insert(job.id.clone(), attempt);
                                drop(counts);

                                // Create error context
                                let context = ErrorContext {
                                    job_id: job.id.clone(),
                                    error: err.clone(),
                                    attempt,
                                    max_attempts: max_retries,
                                };

                                // Ask healer what to do
                                let action = error_healer.heal(&context).await;
                                
                                match action {
                                    HealingAction::Retry => {
                                        println!("Job {} failed (attempt {}), retrying immediately: {}", job.id, attempt, err);
                                        let _ = sender.try_send(job.clone());
                                    }
                                    HealingAction::RetryAfter(ms) => {
                                        println!("Job {} failed (attempt {}), retrying after {}ms: {}", job.id, attempt, ms, err);
                                        let job_clone = job.clone();
                                        let sender_clone = sender.clone();
                                        tokio::spawn(async move {
                                            tokio::time::sleep(tokio::time::Duration::from_millis(ms)).await;
                                            let _ = sender_clone.try_send(job_clone);
                                        });
                                    }
                                    HealingAction::Skip => {
                                        eprintln!("Job {} failed after {} attempts, skipping: {}", job.id, attempt, err);
                                    }
                                    HealingAction::Abort => {
                                        eprintln!("Job {} failed, aborting workflow: {}", job.id, err);
                                        // Could implement graceful shutdown here
                                    }
                                }
                            }
                        }
                        
                        drop(permit);
                        (job.id.clone(), result)
                    });
                }
                Some((job_id, res)) = futures.next() => {
                    match res {
                        Ok(_result) => println!("✓ Job {} succeeded", job_id),
                        Err(_err) => {
                            // Error handling already done above
                        }
                    }
                }
                else => break,
            }
        }
    }
}