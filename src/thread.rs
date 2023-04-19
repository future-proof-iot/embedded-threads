pub use crate::{RunqueueId, ThreadFlags, ThreadId};

use crate::{arch, THREADS};

/// Main struct for holding thread data
#[derive(Debug)]
pub struct Thread {
    pub sp: usize,
    pub state: ThreadState,
    pub prio: RunqueueId,
    pub pid: ThreadId,
    pub flags: ThreadFlags,
    pub(crate) high_regs: [usize; 8],
}

/// Possible states of a thread
#[derive(Copy, Clone, PartialEq, Debug)]
pub enum ThreadState {
    Invalid,
    Running,
    Paused,
    LockBlocked,
    FlagBlocked(crate::thread_flags::WaitMode),
    ChannelRxBlocked(usize),
    ChannelTxBlocked(usize),
    Zombie,
}

impl Thread {
    /// create a default Thread object
    pub const fn default() -> Thread {
        Thread {
            sp: 0,
            state: ThreadState::Invalid,
            high_regs: [0; 8],
            flags: 0,
            prio: 0,
            pid: 0,
        }
    }
}

/// Suspend/pause the current thread's execution
pub fn sleep() {
    THREADS.with_mut(|mut threads| {
        let pid = threads.current_pid().unwrap();
        threads.set_state(pid, ThreadState::Paused);
        arch::schedule();
    });
}
