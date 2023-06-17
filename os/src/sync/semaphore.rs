use crate::sync::UPIntrFreeCell;
use crate::task::{wakeup_task, block_current_and_run_next, 
                current_task, current_process, TaskControlBlock};
use alloc::{collections::VecDeque, sync::Arc};
use crate::task::TaskStatus;
use super::CheckDeadlock;

pub struct Semaphore {
    pub inner: UPIntrFreeCell<SemaphoreInner>,
}

pub struct SemaphoreInner {
    pub count: isize,
    pub wait_queue: VecDeque<Arc<TaskControlBlock>>,
}

impl Semaphore {
    pub fn new(res_count: usize) -> Self {
        Self {
            inner: unsafe {
                UPIntrFreeCell::new(SemaphoreInner {
                    count: res_count as isize,
                    wait_queue: VecDeque::new(),
                })
            },
        }
    }

    pub fn up(&self) {
        let mut inner = self.inner.exclusive_access();
        inner.count += 1;
        if inner.count <= 0 {
            if let Some(task) = inner.wait_queue.pop_front() {
                wakeup_task(task);
            }
        }
    }

    pub fn down(&self) {
        let mut inner = self.inner.exclusive_access();
        inner.count -= 1;
        if inner.count < 0 {
            inner.wait_queue.push_back(current_task().unwrap());
            drop(inner);
            block_current_and_run_next();
        }
    }
}

impl CheckDeadlock for Semaphore {
    fn check_deadlock(&self, request: isize) -> bool {
        let inner = self.inner.exclusive_access();
        if inner.count < request {
            let process = current_process();
            let inner = process.inner_exclusive_access();
            let tasks = inner.tasks.clone();

            let mut flag: bool = true;

            for t in tasks.iter() {
                let task = t.as_ref().unwrap();
                let task_in = task.inner_exclusive_access();
                
                if task_in.task_status == TaskStatus::Ready {
                    flag = false;
                    break;
                }
            }
            return flag;
        }
        false
    }
}
