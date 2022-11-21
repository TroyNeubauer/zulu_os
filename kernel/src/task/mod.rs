pub mod executor;
pub mod keyboard;
pub mod simple_executor;

use alloc::boxed::Box;
use core::sync::atomic::{AtomicU64, Ordering};
use core::task::{Context, Poll};
use core::{future::Future, pin::Pin};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct TaskId(u64);

impl TaskId {
    fn new() -> Self {
        static NEXT_ID: AtomicU64 = AtomicU64::new(0);
        TaskId(NEXT_ID.fetch_add(1, Ordering::Relaxed))
    }
}

pub struct Task {
    future: Pin<Box<dyn Future<Output = ()>>>,
    id: TaskId,
}

impl Task {
    fn poll(&mut self, context: &mut Context) -> Poll<()> {
        self.future.as_mut().poll(context)
    }

    pub fn new(future: impl Future<Output = ()> + 'static) -> Task {
        Task {
            future: Box::pin(future),
            id: TaskId::new(),
        }
    }
}
