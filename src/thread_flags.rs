/// type of thread flags
pub type ThreadFlags = u16;

/// Possible waiting modes for thread flags
#[derive(Copy, Clone, PartialEq, Debug)]
pub enum WaitMode {
    Any(ThreadFlags),
    All(ThreadFlags),
}

// impl Threads {
//      // thread flags implementation
//      fn flag_set(&mut self, thread_id: ThreadId, mask: ThreadFlags) {
//          let mut thread = self.th
//          self.flags |= mask;
//          if match self.state {
//              ThreadState::FlagBlocked(mode) => match mode {
//                  WaitMode::Any(bits) => self.flags & bits != 0,
//                  WaitMode::All(bits) => self.flags & bits == bits,
//              },
//              _ => false,
//          } {
//              s.set_state(ThreadState::Running);
//              Thread::yield_higher();
//          }
//      }
// }

//     pub fn flag_wait_all(mask: ThreadFlags) -> ThreadFlags {
//         let thread = Thread::current();
//         loop {
//             if let Some(result) = cortex_m::interrupt::free(|_| {
//                 if thread.flags & mask != 0 {
//                     let result = thread.flags & mask;
//                     thread.flags &= !mask;
//                     Some(result)
//                 } else {
//                     None
//                 }
//             }) {
//                 return result;
//             } else {
//                 thread.set_state(ThreadState::FlagBlocked(WaitMode::All(mask)));
//                 Thread::yield_higher();
//             }
//         }
//     }

//     pub fn flag_wait_any(mask: ThreadFlags) -> ThreadFlags {
//         let thread = Thread::current();
//         loop {
//             if let Some(result) = cortex_m::interrupt::free(|_| {
//                 if thread.flags & mask != 0 {
//                     let res = thread.flags & mask;
//                     thread.flags &= !res;
//                     Some(res)
//                 } else {
//                     None
//                 }
//             }) {
//                 return result;
//             } else {
//                 thread.set_state(ThreadState::FlagBlocked(WaitMode::Any(mask)));
//                 Thread::yield_higher();
//             }
//         }
//     }

//     pub fn flag_wait_one(mask: ThreadFlags) -> ThreadFlags {
//         let thread = Thread::current();
//         loop {
//             if let Some(result) = cortex_m::interrupt::free(|_| {
//                 if thread.flags & mask != 0 {
//                     let mut res = thread.flags & mask;
//                     // clear all but least significant bit
//                     res &= !res + 1;
//                     thread.flags &= !res;
//                     Some(res)
//                 } else {
//                     None
//                 }
//             }) {
//                 return result;
//             } else {
//                 thread.set_state(ThreadState::FlagBlocked(WaitMode::Any(mask)));
//                 Thread::yield_higher();
//             }
//         }
//     }

//     pub fn flag_clear(mask: ThreadFlags) -> ThreadFlags {
//         let thread = Thread::current();
//         cortex_m::interrupt::free(|_| {
//             let res = thread.flags & mask;
//             thread.flags &= !mask;
//             res
//         })
//     }
// }
