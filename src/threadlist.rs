use crate::arch::interrupt::CriticalSection;

use crate::{ThreadId, Threads};

pub(crate) struct ThreadList {
    pub head: Option<ThreadId>,
}

impl ThreadList {
    pub(crate) const fn new() -> Self {
        Self { head: None }
    }

    pub(crate) fn pop(&mut self, cs: &CriticalSection) -> Option<ThreadId> {
        if let Some(head) = self.head {
            let result = self.head;
            let thread = unsafe { &Threads::get_mut(cs).threads[head as usize] };
            self.head = thread.next;
            result
        } else {
            None
        }
    }
}
