use super::{Task, TaskId};
use alloc::{collections::BTreeMap, sync::Arc, task::Wake};
use core::task::{Context, Poll, Waker};
use crossbeam_queue::ArrayQueue;

pub struct Executor {
    tasks: BTreeMap<TaskId, Task>,
    task_queue: Arc<ArrayQueue<TaskId>>,
    waker_cache: BTreeMap<TaskId, Waker>,
}

const MAX_TASKS: usize = 128;

impl Executor {
    pub fn new() -> Self {
        Self {
            tasks: BTreeMap::new(),
            task_queue: Arc::new(ArrayQueue::new(MAX_TASKS)),
            waker_cache: BTreeMap::new(),
        }
    }

    pub fn spawn(&mut self, task: Task) {
        let id = task.id;
        if self.tasks.insert(id, task).is_some() {
            panic!("Task ID overflow: {:?}", id);
        }
        self.task_queue.push(id).expect("Task queue full");
    }

    fn run_ready_tasks(&mut self) {
        crate::println!("There are {} tasks", self.task_queue.len());
        while let Ok(id) = self.task_queue.pop() {
            let task = match self.tasks.get_mut(&id) {
                Some(t) => t,
                None => continue, //Task already stopped
            };
            let waker = self
                .waker_cache
                .entry(id)
                .or_insert_with(|| TaskWaker::new(id, Arc::clone(&self.task_queue)));

            let mut context = Context::from_waker(waker);
            match task.poll(&mut context) {
                Poll::Ready(()) => {
                    self.tasks.remove(&id);
                    self.waker_cache.remove(&id);
                }
                Poll::Pending => {
                    //Nop. The contract of waker means that the task will push to `self.task_queue`
                    //when it wants to be re-polled
                }
            }
        }
    }

    pub fn run(&mut self) -> ! {
        loop {
            self.run_ready_tasks();
            self.sleep_if_idle();
        }
    }

    fn sleep_if_idle(&self) {
        crate::sys::wait_for_interrupts_if(|| {
            let b = self.task_queue.is_empty();
            if b {
                crate::println!("Sleeping");
            }
            b
        });
    }
}

struct TaskWaker {
    id: TaskId,
    task_queue: Arc<ArrayQueue<TaskId>>,
}

impl TaskWaker {
    fn new(id: TaskId, task_queue: Arc<ArrayQueue<TaskId>>) -> Waker {
        Waker::from(Arc::new(Self { id, task_queue }))
    }

    fn wake_task(&self) {
        self.task_queue.push(self.id).expect("Task queue full");
    }
}

impl Wake for TaskWaker {
    fn wake(self: Arc<Self>) {
        self.wake_task();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        self.wake_task();
    }
}
