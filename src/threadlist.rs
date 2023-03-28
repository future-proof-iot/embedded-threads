use critical_section::CriticalSection;

use crate::{arch, ThreadId, ThreadState, THREADS};

pub(crate) struct ThreadList {
    pub head: Option<ThreadId>,
}

impl ThreadList {
    pub(crate) const fn new() -> Self {
        Self { head: None }
    }

    pub(crate) fn put_current(&mut self, cs: CriticalSection) {
        THREADS.with_mut_cs(cs, |mut threads| {
            let thread_id = threads.current_thread.unwrap();
            threads.thread_blocklist[thread_id as usize] = self.head;
            self.head = Some(thread_id);
            threads.set_state(thread_id, crate::ThreadState::LockBlocked);
            arch::schedule();
        });
    }

    pub(crate) fn pop(&mut self, cs: CriticalSection) -> Option<ThreadId> {
        if let Some(head) = self.head {
            let result = Some(head);
            THREADS.with_mut_cs(cs, |mut threads| {
                self.head = threads.thread_blocklist[head as usize].take();
                threads.set_state(head, ThreadState::Running);
                arch::schedule();
            });
            result
        } else {
            None
        }
    }
}
