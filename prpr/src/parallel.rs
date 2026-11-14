
use once_cell::sync::Lazy;
use std::sync::{mpsc, Arc, Condvar, Mutex};
use std::thread;

pub fn worker_count() -> usize {
    thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
        .max(1)
}

pub static POOL: Lazy<ThreadPool> = Lazy::new(|| ThreadPool::new(worker_count()));

struct PoolMsg {

    data: *mut (),

    run: unsafe fn(*mut ()),
    pending: Arc<(Mutex<usize>, Condvar)>,
}

unsafe impl Send for PoolMsg {}

pub struct ThreadPool {

    txs: Vec<mpsc::SyncSender<PoolMsg>>,
    workers: usize,
}

impl ThreadPool {
    pub fn new(workers: usize) -> Self {
        let workers = workers.max(1);
        let mut txs = Vec::with_capacity(workers);
        for _ in 0..workers {
            let (tx, rx): (mpsc::SyncSender<PoolMsg>, mpsc::Receiver<PoolMsg>) = mpsc::sync_channel(1);
            txs.push(tx);
            thread::spawn(move || loop {
                let msg = match rx.recv() {
                    Ok(msg) => msg,
                    Err(_) => break,
                };

                let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe { (msg.run)(msg.data) }));
                let (lock, cvar) = &*msg.pending;
                let mut n = lock.lock().unwrap();
                *n -= 1;
                if *n == 0 {
                    cvar.notify_all();
                }
            });
        }
        Self { txs, workers }
    }

    pub fn worker_count(&self) -> usize {
        self.workers
    }

    pub fn scoped_parallel_for<T, F>(&self, items: &mut [T], f: F)
    where
        T: Send,
        F: Fn(&mut T) + Sync,
    {
        let n = items.len();
        let workers = self.workers.min(n).max(1);
        if workers <= 1 || n <= 1 {
            for it in items {
                f(it);
            }
            return;
        }
        let chunk = n.div_ceil(workers);

        struct JobCtx<T, F> {
            items: *mut T,
            start: usize,
            end: usize,
            f: *const F,
        }
        let mut ctxs: Vec<JobCtx<T, F>> = Vec::with_capacity(workers);
        for start in (0..n).step_by(chunk) {
            let end = (start + chunk).min(n);
            ctxs.push(JobCtx {
                items: items.as_mut_ptr(),
                start,
                end,
                f: &f,
            });
        }

        unsafe fn job_runner<T: Send, F: Fn(&mut T) + Sync>(data: *mut ()) {
            let ctx = unsafe { &mut *(data as *mut JobCtx<T, F>) };
            for i in ctx.start..ctx.end {
                let item = unsafe { &mut *ctx.items.add(i) };
                (*ctx.f)(item);
            }
        }
        let pending = Arc::new((Mutex::new(ctxs.len()), Condvar::new()));
        {
            let (lock, _) = &*pending;
            *lock.lock().unwrap() = ctxs.len();
        }
        for (tx, ctx) in self.txs.iter().zip(ctxs.iter_mut()) {
            tx.send(PoolMsg {
                data: ctx as *mut JobCtx<T, F> as *mut (),
                run: job_runner::<T, F>,
                pending: Arc::clone(&pending),
            })
            .expect("thread pool workers should be alive");
        }

        let (lock, cvar) = &*pending;
        let mut n = lock.lock().unwrap();
        while *n > 0 {
            n = cvar.wait(n).unwrap();
        }
    }
}

pub fn parallel_for<T, F>(items: &mut [T], f: F)
where
    T: Send,
    F: Fn(&mut T) + Sync,
{
    let workers = worker_count();
    if workers <= 1 || items.len() <= 1 {
        for it in items {
            f(it);
        }
        return;
    }
    let chunk = items.len().div_ceil(workers);
    let f = &f;
    thread::scope(|s| {
        for slice in items.chunks_mut(chunk) {
            s.spawn(move || {
                for it in slice {
                    f(it);
                }
            });
        }
    });
}

pub fn parallel_map<T, R, F>(items: Vec<T>, f: F) -> Vec<R>
where
    T: Send,
    R: Send,
    F: Fn(T) -> R + Sync,
{
    let n = items.len();
    let workers = worker_count();
    if workers <= 1 || n <= 1 {
        return items.into_iter().map(f).collect();
    }
    let chunk = n.div_ceil(workers);
    let mut items = items;
    let mut slots: Vec<Option<Vec<R>>> = (0..workers).map(|_| None).collect();
    thread::scope(|s| {
        let mut cursor = 0usize;
        for slot in slots.iter_mut() {
            let take = chunk.min(n - cursor);
            if take == 0 {
                break;
            }
            let rest = items.split_off(take);
            let part = std::mem::replace(&mut items, rest);
            let f = &f;
            s.spawn(move || {
                *slot = Some(part.into_iter().map(|it| f(it)).collect());
            });
            cursor += take;
        }
    });
    let mut out = Vec::with_capacity(n);
    for slot in slots {

        if let Some(v) = slot {
            out.extend(v);
        }
    }
    out
}
