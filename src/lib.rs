use std::{
    sync::{
        mpsc::{channel, Receiver, Sender},
        Arc, Mutex, atomic::{AtomicBool, self, AtomicI32},
    },
    thread::{self, JoinHandle, sleep}, time::Duration,
};

type Task = Box<dyn RunnableBox + Send + 'static>;

pub(crate) enum MessageToWorker {
    Work(Task),
    WorkNotify(Task, Arc<AtomicBool>),
    Stop,
}

pub trait RunnableBox {
    fn run(self: Box<Self>);
}

impl<F: FnOnce()> RunnableBox for F {
    fn run(self: Box<Self>) {
        (*self)()
    }
}

#[derive(Default)]
pub(crate) struct Worker {
    #[allow(dead_code)] // Is used for debugging
    id: usize,
    thread: Option<JoinHandle<()>>,
}

impl Worker {
    pub(crate) fn new(id: usize, rec: Arc<Mutex<Receiver<MessageToWorker>>>) -> Self {
        let thread = thread::spawn(move || loop {
            let message = rec.lock().unwrap().recv().unwrap();
            
            match message {
                MessageToWorker::Work(job) => {
                    job.run();
                }
                MessageToWorker::WorkNotify(job, token) => {
                    job.run();
                    token.swap(true, atomic::Ordering::Relaxed);
                }
                MessageToWorker::Stop => {
                    break;
                }
            }
        });

        Worker {
            id,
            thread: Some(thread),
        }
    }
}

pub struct ThreadPool {
    workers: Vec<Worker>,
    sender: Sender<MessageToWorker>,
}

// TODO: Add future/task support? FnMut(T) -> Task<U>
impl ThreadPool {
    pub fn new(number_of_workers: usize) -> Self {
        let (sender, rec) = channel();
        let receiver = Arc::new(Mutex::new(rec));

        let mut workers = Vec::with_capacity(number_of_workers);
        for id in 0..=number_of_workers {
            workers.push(Worker::new(id, receiver.clone()))
        }

        ThreadPool { workers, sender }
    }

    pub fn run<T>(&mut self, func: T)
    where
        T: FnMut() + Send + 'static,
    {
        self.sender
            .send(MessageToWorker::Work(Box::new(func)))
            .unwrap()
    }

    pub fn run_await<T>(&mut self, func: T)
    where
    T: FnMut() + Send + 'static,
    {
        let finished = Arc::new(AtomicBool::new(false));
        self.sender
            .send(MessageToWorker::WorkNotify(Box::new(func), finished.clone()))
            .unwrap();
        while !finished.load(atomic::Ordering::Relaxed) {
            sleep(Duration::from_millis(1));
        }
    }

    pub fn run_many<T>(&mut self, funcs: Vec<T>)
    where
        T: FnMut() + Send + 'static,
    {
        for func in funcs {
            self.sender
                .send(MessageToWorker::Work(Box::new(func)))
                .unwrap()
        }
    }

    pub fn run_many_await<T>(&mut self, funcs: Vec<T>)
    where
    T: FnMut() + Send + 'static,
    {
        let mut finished: Vec<Arc<AtomicBool>> = Vec::new();
        for func in funcs {
            let func_finished = Arc::new(AtomicBool::new(false));
            self.sender
                .send(MessageToWorker::WorkNotify(Box::new(func), func_finished.clone()))
                .unwrap();
            finished.push(func_finished);
        }
        while !Iterator::all(&mut finished.iter(), |x| {
            x.load(atomic::Ordering::Relaxed)
        }) {
            sleep(Duration::from_millis(1));
        }
    }

}

impl Drop for ThreadPool {
    fn drop(&mut self) {
        for _ in &mut self.workers {
            self.sender.send(MessageToWorker::Stop).unwrap()
        }

        for worker in &mut self.workers {
            if let Some(thread) = worker.thread.take() {
                thread.join().unwrap()
            }
        }
    }
}

#[test]
pub fn run_many_await_is_sequential_at_call_site() {
    let mut pool = ThreadPool::new(4);
    let mut tasks = Vec::new();
    let count = 100;

    let mut_me = Arc::new(AtomicI32::new(0));
    for _ in 0..count {
        let clone = mut_me.clone();
        let task = move || {
            clone.fetch_add(1, atomic::Ordering::Relaxed);
        };
        tasks.push(task);
    }
    pool.run_many_await(tasks);
    assert!(mut_me.load(atomic::Ordering::Relaxed) == count);
    mut_me.fetch_add(1, atomic::Ordering::Relaxed);
    assert!(mut_me.load(atomic::Ordering::Relaxed) == count + 1)
}