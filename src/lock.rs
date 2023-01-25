use core::cell::UnsafeCell;

use super::arch::interrupt;
use super::threadlist::ThreadList;
use super::{ThreadState, Threads};

pub struct Lock {
    state: UnsafeCell<LockState>,
}

enum LockState {
    Unlocked,
    Locked(ThreadList),
}

impl Lock {
    pub const fn new() -> Self {
        Self {
            state: UnsafeCell::new(LockState::Unlocked),
        }
    }

    pub const fn new_locked() -> Self {
        Self {
            state: UnsafeCell::new(LockState::Locked(ThreadList::new())),
        }
    }

    pub fn is_locked(&self) -> bool {
        interrupt::free(|_| {
            let state = unsafe { &*self.state.get() };
            match state {
                LockState::Unlocked => false,
                _ => true,
            }
        })
    }

    pub fn acquire(&self) {
        interrupt::free(|cs| {
            let state = unsafe { &mut *self.state.get() };
            match state {
                LockState::Unlocked => *state = LockState::Locked(ThreadList::new()),
                LockState::Locked(waiters) => {
                    unsafe { Threads::get_mut(cs) }.current_wait_on(waiters, ThreadState::LockWait);
                }
            }
        })
    }

    pub fn try_acquire(&self) -> bool {
        interrupt::free(|_| {
            let state = unsafe { &mut *self.state.get() };
            match state {
                LockState::Unlocked => {
                    *state = LockState::Locked(ThreadList::new());
                    true
                }
                LockState::Locked(_) => false,
            }
        })
    }

    pub fn release(&self) {
        interrupt::free(|cs| {
            let state = unsafe { &mut *self.state.get() };
            match state {
                LockState::Unlocked => {} // TODO: panic?
                LockState::Locked(waiters) => {
                    if let Some(thread_id) = waiters.pop(cs) {
                        //super::println!("unlocking {}", thread_id);
                        unsafe { Threads::get_mut(cs) }.wake_pid(thread_id);
                    } else {
                        *state = LockState::Unlocked
                    }
                }
            }
        })
    }
}
