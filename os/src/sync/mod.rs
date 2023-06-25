mod condvar;
mod mutex;
mod semaphore;
mod up;

pub use condvar::Condvar;
pub use mutex::{Mutex, MutexBlocking, MutexSpin};
pub use semaphore::Semaphore;
pub use up::{UPIntrFreeCell, UPIntrRefMut};

pub trait CheckDeadlock {
    fn check_deadlock(&self, request: isize) -> bool;
}
