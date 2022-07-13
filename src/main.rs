use std::{
    io::stdin,
    sync::{
        mpsc::{channel, Receiver, Sender},
        Arc, Mutex,
    },
    thread::{self, JoinHandle},
};

type Task = Box<dyn RunnableBox + Send + 'static>;

enum MessageToWorker {
    Work(Task),
    WorkNotify(Task, Arc<bool>),
    Stop,
}

trait RunnableBox {
    fn run(self: Box<Self>);
}

impl<F: FnOnce()> RunnableBox for F {
    fn run(self: Box<Self>) {
        (*self)()
    }
}

#[derive(Default)]
struct Worker {
    #[allow(dead_code)] // Is used for debugging
    id: usize,
    thread: Option<JoinHandle<()>>,
}

impl Worker {
    fn new(id: usize, rec: Arc<Mutex<Receiver<MessageToWorker>>>) -> Self {
        let thread = thread::spawn(move || loop {
            let message = rec.lock().unwrap().recv().unwrap();

            match message {
                MessageToWorker::Work(job) => {
                    println!("Worker {} got a job; executing.", id);

                    job.run();
                }
                MessageToWorker::WorkNotify(job, token) => {
                    println!("Worker {} got a async job; executing.", id);
                    
                    job.run();
                    *token = true;
                }
                MessageToWorker::Stop => {
                    println!("Worker {} was told to terminate.", id);

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

struct ThreadPool {
    workers: Vec<Worker>,
    sender: Sender<MessageToWorker>,
}

// TODO: Add "awaitable" tasks, opaque "ticket" and new message type with response?
impl ThreadPool {
    fn new(number_of_workers: usize) -> Self {
        let (sender, rec) = channel();
        let receiver = Arc::new(Mutex::new(rec));

        let mut workers = Vec::with_capacity(number_of_workers);
        for id in 0..=number_of_workers {
            workers.push(Worker::new(id, receiver.clone()))
        }

        ThreadPool { workers, sender }
    }

    fn run<T>(&mut self, func: T)
    where
        T: FnMut() + Send + 'static,
    {
        self.sender
            .send(MessageToWorker::Work(Box::new(func)))
            .unwrap()
    }

    fn run_await<T>(&mut self, func: T)
    where
    T: FnMut() + Send + 'static,
    {
        self.sender
            .send(MessageToWorker::Work(Box::new(func)))
            .unwrap();
        self.sender.
    }

    fn run_many<T>(&mut self, funcs: Vec<T>)
    where
        T: FnMut() + Send + 'static,
    {
        for func in funcs {
            self.sender
                .send(MessageToWorker::Work(Box::new(func)))
                .unwrap()
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

fn main() {
    let mut pool = ThreadPool::new(8);
    pool.run(|| println!("Hello world"));

    loop {
        let mut input = String::new();
        stdin().read_line(&mut input).unwrap();
        if input.starts_with("exit") {
            break;
        }
        if input.starts_with("add") {
            let ws_split: Vec<&str> = input.split_ascii_whitespace().collect();
            let num1 = ws_split[1].parse::<usize>().unwrap();
            let num2 = ws_split[2].parse::<usize>().unwrap();
            pool.run(move || {
                let res = num1 + num2;
                println!("Result of {} + {} = {}", num1, num2, res)
            });
        } else {
            pool.run(move || println!("{}", input));
        }
    }
    drop(pool)
}
