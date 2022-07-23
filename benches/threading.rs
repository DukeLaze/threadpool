use std::sync::{atomic::{AtomicI32, self}, Arc};

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use threadpool::ThreadPool;

pub fn criterion_benchmark(c: &mut Criterion) {
    let count = 100_000;
    for i in 1..=4 {
        c.bench_function(format!("await_many_{}t", i).as_str(), |b| {
            let mut tasks = Vec::new();
            let mut_me = Arc::new(AtomicI32::new(0));
            for _ in 0..count {
                let clone = mut_me.clone();
                let task = move || {
                    clone.fetch_add(1, atomic::Ordering::Relaxed);
                };
                tasks.push(task);
            }
            let mut pool = ThreadPool::new(i);
            b.iter(|| pool.run_many_await(tasks.clone()))
        });
        c.bench_function(format!("run_many_{}t", i).as_str(), |b| {
            let mut pool = ThreadPool::new(i);
            let mut tasks = Vec::new();
            for _ in 0..count {
                let task = move || {
                    let _ = (i+i*99)*99+9;
                };
                tasks.push(task);
            }
            b.iter(|| pool.run_many(tasks.clone()))
        });
    }
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);