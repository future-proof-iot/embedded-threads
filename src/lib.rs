#![no_std]
#![feature(inline_const)]
#![feature(naked_functions)]

use core::arch::asm;

use cortex_m_semihosting::hprintln as println;
use critical_section::CriticalSection;

use riot_rs_runqueue::RunQueue;
pub use riot_rs_runqueue::{RunqueueId, ThreadId};

mod arch;
mod ensure_once;
mod threadlist;

pub mod lock;
pub mod thread_flags;

pub use arch::schedule;

use ensure_once::EnsureOnce;
pub use thread_flags::*;

/// global defining the number of possible priority levels
pub const SCHED_PRIO_LEVELS: usize = 8;

/// global defining the number of threads that can be created
pub const THREADS_NUMOF: usize = 8;

pub(crate) static THREADS: EnsureOnce<Threads> = EnsureOnce::new(Threads::new());

/// Main struct for holding thread data
#[derive(Debug)]
pub struct Thread {
    pub sp: usize,
    pub state: ThreadState,
    pub prio: RunqueueId,
    pub pid: ThreadId,
    pub flags: ThreadFlags,
    high_regs: [usize; 8],
}

pub struct Threads {
    /// global thread runqueue
    runqueue: RunQueue<SCHED_PRIO_LEVELS, THREADS_NUMOF>,
    threads: [Thread; THREADS_NUMOF],
    thread_blocklist: [Option<ThreadId>; THREADS_NUMOF],
    current_thread: Option<ThreadId>,
}

/// Possible states of a thread
#[derive(Copy, Clone, PartialEq, Debug)]
pub enum ThreadState {
    Invalid,
    Running,
    Paused,
    LockBlocked,
    FlagBlocked(thread_flags::WaitMode),
}

impl Threads {
    const fn new() -> Self {
        Self {
            runqueue: RunQueue::new(),
            threads: [const { Thread::default() }; THREADS_NUMOF],
            thread_blocklist: [const { None }; THREADS_NUMOF],
            current_thread: None,
        }
    }

    pub(crate) fn by_pid_unckecked(&mut self, thread_id: ThreadId) -> &mut Thread {
        &mut self.threads[thread_id as usize]
    }

    pub(crate) fn current(&mut self) -> Option<&mut Thread> {
        self.current_thread
            .map(|tid| &mut self.threads[tid as usize])
    }

    pub fn current_pid(&self) -> Option<ThreadId> {
        self.current_thread
    }

    /// Create a new thread
    pub(crate) fn create(
        &mut self,
        func: usize,
        arg: usize,
        stack: &mut [u8],
        prio: u8,
    ) -> Option<&mut Thread> {
        if let Some((thread, pid)) = self.get_unused() {
            thread.sp = arch::setup_stack(stack, func, arg);
            thread.prio = prio;
            thread.pid = pid;
            thread.state = ThreadState::Paused;

            Some(thread)
        } else {
            None
        }
    }

    fn get_unchecked(&self, thread_id: ThreadId) -> &Thread {
        &self.threads[thread_id as usize]
    }

    fn get_unchecked_mut(&mut self, thread_id: ThreadId) -> &mut Thread {
        &mut self.threads[thread_id as usize]
    }

    // get an unused ThreadId / Thread slot
    fn get_unused(&mut self) -> Option<(&mut Thread, ThreadId)> {
        for i in 0..THREADS_NUMOF {
            if self.threads[i].state == ThreadState::Invalid {
                return Some((&mut self.threads[i], i as ThreadId));
            }
        }
        None
    }

    /// set state of thread
    ///
    /// This function handles adding/removing the thread to the Runqueue depending
    /// on its previous or new state.
    pub(crate) fn set_state(&mut self, pid: ThreadId, state: ThreadState) {
        let thread = &mut self.threads[pid as usize];
        let old_state = thread.state;
        thread.state = state;
        if old_state != ThreadState::Running && state == ThreadState::Running {
            //println!("adding {} to runqueue", thread.pid);

            self.runqueue.add(thread.pid, thread.prio);
        } else if old_state == ThreadState::Running && state != ThreadState::Running {
            self.runqueue.del(thread.pid, thread.prio);
        }
    }
}

/// start threading
///
/// Supposed to be started early on by OS startup code.
///
/// # Safety
/// This may only be called once.
pub unsafe fn start_threading() {
    // faking a critical section to get THREADS
    let cs = CriticalSection::new();
    let next_sp = THREADS.with_mut_cs(cs, |mut threads| {
        let next_pid = threads.runqueue.get_next().unwrap();
        threads.current_thread = Some(next_pid);
        threads.threads[next_pid as usize].sp
    });
    arch::start_threading(next_sp);
}

/// scheduler
#[no_mangle]
unsafe fn sched(old_sp: usize) {
    let cs = CriticalSection::new();
    let next_pid;

    loop {
        {
            if let Some(pid) = (&*THREADS.as_ptr(cs)).runqueue.get_next() {
                next_pid = pid;
                break;
            }
        }
        //pm_set_lowest();
        cortex_m::interrupt::enable();
        // pending interrupts would now get to run their ISRs
        cortex_m::interrupt::disable();
    }

    let mut threads = &mut *THREADS.as_ptr(cs);
    let current_high_regs;

    if let Some(current_pid) = threads.current_pid() {
        if next_pid == current_pid {
            asm!("", in("r0") 0);
            return;
        }
        //println!("current: {} next: {}", current_pid, next_pid);
        threads.threads[current_pid as usize].sp = old_sp;
        threads.current_thread = Some(next_pid);
        current_high_regs = threads.threads[current_pid as usize].high_regs.as_ptr();
    } else {
        current_high_regs = core::ptr::null();
    }

    let next = &threads.threads[next_pid as usize];
    let next_sp = next.sp;
    let next_high_regs = next.high_regs.as_ptr();

    //println!("old_sp: {:x} next.sp: {:x}", old_sp, next_sp);

    // PendSV expects these three pointers in r0, r1 and r2:
    // r0= &current.high_regs
    // r1= &next.high_regs
    // r2= &next.sp
    //
    // write to registers manually, as ABI would return the values via stack
    asm!("", in("r0") current_high_regs, in("r1") next_high_regs, in("r2")next_sp);
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

/// trait for types that fit into a single register.
///
/// Currently implemented for references (`&T`) and usize.
pub trait Arguable {
    fn into_arg(self) -> usize;
}

impl Arguable for usize {
    fn into_arg(self) -> usize {
        self
    }
}

impl Arguable for () {
    fn into_arg(self) -> usize {
        0
    }
}

impl<T> Arguable for &T {
    fn into_arg(self) -> usize {
        self as *const T as usize
    }
}

pub fn thread_create<T: Arguable + Send>(func: fn(arg: T), arg: T, stack: &mut [u8], prio: u8) {
    let arg = arg.into_arg();
    thread_create_raw(func as usize, arg, stack, prio)
}

pub fn thread_create_raw(func: usize, arg: usize, stack: &mut [u8], prio: u8) {
    THREADS.with_mut(|mut threads| {
        let pid = threads.create(func, arg, stack, prio).unwrap().pid;
        threads.set_state(pid, ThreadState::Running);
    });
}

pub fn current_pid() -> Option<ThreadId> {
    THREADS.with(|threads| threads.current_pid())
}

/// thread cleanup function
///
/// This gets hooked into a newly created thread stack so it gets called when
/// the thread function returns.
fn cleanup() -> ! {
    let pid = THREADS.with_mut(|mut threads| {
        let thread_id = threads.current_pid().unwrap();
        threads.set_state(thread_id, ThreadState::Invalid);
        thread_id
    });

    println!("thread {}: exited", pid);

    arch::schedule();

    unreachable!();
}
