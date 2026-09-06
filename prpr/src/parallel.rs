//! 并行工具
//!
//! 使用**全部**逻辑核心（例如 20 逻辑核心 → 20 个工作线程）。
//!
//! - [`ThreadPool::scoped_parallel_for`]：**持久线程池**，工作线程常驻，
//!   用于每帧的批量运算（判定线推进、Note 负载统计），避免每帧创建线程的开销
//! - [`parallel_map`] / [`parallel_for`]：一次性 scoped 并行映射，用于谱面解析等
//!   批量工作（解析时按线并行）
//!
//! 线程池的实现采用"调用方阻塞屏障 + 原始指针"的标准模式：
//! `scoped_parallel_for` 阻塞直到所有工作线程完成，因此任务闭包对调用方数据的
//! 借用在整个调用期间始终有效，这是 [`PoolMsg`] 中裸指针可以跨线程的前提。

use once_cell::sync::Lazy;
use std::sync::{mpsc, Arc, Condvar, Mutex};
use std::thread;

/// 参与并行运算的工作线程数：全部逻辑核心（至少 1）。
pub fn worker_count() -> usize {
    thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
        .max(1)
}

/// 全局持久线程池（工作线程数 = 全部逻辑核心），首次使用时创建。
pub static POOL: Lazy<ThreadPool> = Lazy::new(|| ThreadPool::new(worker_count()));

struct PoolMsg {
    /// 任务上下文指针（指向调用方栈上的 `JobCtx`），生命周期由调用方的阻塞等待保证
    data: *mut (),
    /// 任务执行函数（泛型实例化的函数指针，`'static` 无生命周期问题）
    run: unsafe fn(*mut ()),
    pending: Arc<(Mutex<usize>, Condvar)>,
}

// 裸指针 + 调用方阻塞屏障：PoolMsg 在被等待期间始终有效，因此可安全跨线程发送
unsafe impl Send for PoolMsg {}

/// 常驻工作线程的线程池。
pub struct ThreadPool {
    /// 每个工作线程一条独立通道（mpsc 的 Receiver 不可克隆）
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
                    Err(_) => break, // 通道关闭，线程退出
                };
                // 安全：调用方阻塞等待 pending 归零后才返回，期间 msg.data 指向的
                // 上下文与数据仍有效（调用方屏障）。用 catch_unwind 兜底：
                // 即使任务 panic 也递减计数，避免调用方永久阻塞。
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

    /// 将 `items` 分块并行处理（每块一个工作线程，块内保持顺序）。
    ///
    /// **阻塞**直到所有工作线程完成；`items` 与 `f` 的借用因此在整个调用期间
    /// 保持有效（调用方屏障）。工作线程数不足或元素过少时退化为串行。
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
        // 每个工作线程一个上下文（含其负责的下标区间）；上下文与 items/f 都位于
        // 调用方栈上，在下方阻塞等待期间保持有效
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
        // 泛型实例化的执行函数：把 data 解释为 JobCtx，处理 [start, end) 区间
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
        // 阻塞等待全部完成
        let (lock, cvar) = &*pending;
        let mut n = lock.lock().unwrap();
        while *n > 0 {
            n = cvar.wait(n).unwrap();
        }
    }
}

/// 将 `items` 分块并行处理（每块一个工作线程），并保持输入顺序。
///
/// 一次性 scoped 线程（适用于解析等不频繁的批量工作）。
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

/// 将 `items` 分块并行映射，返回与输入顺序一致的结果。
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
        // 块数可能少于工作线程数，未使用的槽位保持 None
        if let Some(v) = slot {
            out.extend(v);
        }
    }
    out
}
