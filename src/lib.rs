#![no_std]
#![feature(inline_const)]
#![feature(naked_functions)]

use core::arch::asm;
use core::cell::UnsafeCell;
use core::ptr::write_volatile;

use cortex_m_semihosting::hprintln as println;

use riot_rs_runqueue::{RunQueue, RunqueueId, ThreadId};

mod arch;
pub use arch::{interrupt, CriticalSection, Mutex};

/// global defining the number of possible priority levels
pub const SCHED_PRIO_LEVELS: usize = 8;

/// global defining the number of threads that can be created
pub const THREADS_NUMOF: usize = 8;

pub static THREADS: Mutex<UnsafeCell<Threads>> = Mutex::new(UnsafeCell::new(Threads::new()));

pub struct Threads {
    /// global thread runqueue
    runqueue: RunQueue<SCHED_PRIO_LEVELS, THREADS_NUMOF>,
    threads: [Thread; THREADS_NUMOF],
    current_thread: Option<ThreadId>,
}

impl Threads {
    const fn new() -> Self {
        Self {
            runqueue: RunQueue::new(),
            threads: [const { Thread::default() }; THREADS_NUMOF],
            current_thread: None,
        }
    }

    /// get the global THREADS list, mutable
    #[allow(clippy::mut_from_ref)]
    pub(crate) unsafe fn get_mut(cs: &CriticalSection) -> &mut Threads {
        &mut *THREADS.borrow(cs).get()
    }

    pub(crate) fn get(cs: &CriticalSection) -> &Threads {
        unsafe { &*THREADS.borrow(cs).get() }
    }

    pub fn current(&mut self) -> Option<&mut Thread> {
        self.current_thread
            .map(|tid| &mut self.threads[tid as usize])
    }

    pub fn current_pid(&self) -> Option<ThreadId> {
        self.current_thread
    }

    fn get_unused(&mut self) -> Option<(&mut Thread, ThreadId)> {
        for i in 0..THREADS_NUMOF {
            if self.threads[i].state == ThreadState::Invalid {
                return Some((&mut self.threads[i], i as ThreadId));
            }
        }
        None
    }

    /// Create a new thread
    pub fn create(
        &mut self,
        func: fn(arg: usize),
        arg: usize,
        stack: &mut [u8],
        prio: u8,
    ) -> Option<&mut Thread> {
        if let Some((thread, pid)) = self.get_unused() {
            thread.sp = Thread::setup_stack(stack, func as usize, arg);
            thread.prio = prio;
            thread.pid = pid;
            thread.state = ThreadState::Paused;

            Some(thread)
        } else {
            None
        }
    }

    /// set state of thread
    ///
    /// This function handles adding/removing the thread to the Runqueue depending
    /// on its previous or new state.
    pub fn set_state(&mut self, pid: ThreadId, state: ThreadState) {
        let thread = &mut self.threads[pid as usize];
        let old_state = thread.state;
        thread.state = state;
        if old_state != ThreadState::Running && state == ThreadState::Running {
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
    let threads = Threads::get_mut(&cs);

    let next_pid = threads.runqueue.get_next().unwrap();
    threads.current_thread = Some(next_pid);
    let next_sp = threads.threads[next_pid as usize].sp;
    asm!(
            "
            msr psp, r1 // set new thread's SP to PSP
            svc 0       // SVC 0 handles switching
            ",
        in("r1")next_sp);
}

/// scheduler
#[no_mangle]
unsafe fn sched(_old_sp: usize) {
    let next_pid;

    loop {
        {
            let cs = CriticalSection::new();

            let threads = Threads::get_mut(&cs);
            if let Some(pid) = threads.runqueue.get_next() {
                next_pid = pid;
                break;
            }
        }
        //pm_set_lowest();
        cortex_m::interrupt::enable();
        // pending interrupts would now get to run their ISRs
        cortex_m::interrupt::disable();
    }

    let cs = CriticalSection::new();

    let threads = Threads::get_mut(&cs);

    let current_high_regs;

    if let Some(current_pid) = threads.current_pid() {
        if next_pid == current_pid {
            asm!("", in("r0") 0);
            return;
        }
        threads.current_thread = Some(next_pid);
        current_high_regs = threads.threads[current_pid as usize].high_regs.as_ptr();
    } else {
        current_high_regs = core::ptr::null();
    }
    let next = &threads.threads[next_pid as usize];

    // PendSV expects these three pointers in r0, r1 and r2:
    // r0= &current.high_regs
    // r1= &next.high_regs
    // r2= &next.sp
    //
    // write to registers manually, as ABI would return the values via stack
    asm!("", in("r0") current_high_regs, in("r1") next.high_regs.as_ptr(), in("r2")next.sp);
}

//}

/// Main struct for holding thread data
#[derive(Debug)]
pub struct Thread {
    sp: usize,
    high_regs: [usize; 8],
    pub(crate) state: ThreadState,
    pub prio: RunqueueId,
    pub pid: ThreadId,
}

/// Possible states of a thread
#[derive(Copy, Clone, PartialEq, Debug)]
pub enum ThreadState {
    Invalid,
    Running,
    Paused,
}

impl Thread {
    /// create a default Thread object
    pub const fn default() -> Thread {
        Thread {
            sp: 0,
            state: ThreadState::Invalid,
            high_regs: [0; 8],
            prio: 0,
            pid: 0,
        }
    }

    /// Sets up stack for newly created threads.
    ///
    /// After running this, the stack should look as if the thread was
    /// interrupted by an ISR. On the next return, it starts executing
    /// `func`.
    fn setup_stack(stack: &mut [u8], func: usize, arg: usize) -> usize {
        let stack_start = stack.as_ptr() as usize;
        let stack_pos = (stack_start + stack.len() - 36) as *mut usize;

        unsafe {
            write_volatile(stack_pos.offset(0), arg); // -> R0
            write_volatile(stack_pos.offset(1), 1); // -> R1
            write_volatile(stack_pos.offset(2), 2); // -> R2
            write_volatile(stack_pos.offset(3), 3); // -> R3
            write_volatile(stack_pos.offset(4), 12); // -> R12
            write_volatile(stack_pos.offset(5), cleanup as usize); // -> LR
            write_volatile(stack_pos.offset(6), func); // -> PC
            write_volatile(stack_pos.offset(7), 0x01000000); // -> APSR
        }

        stack_pos as usize
    }
}

pub fn thread_create(func: fn(arg: usize), arg: usize, stack: &mut [u8], prio: u8) {
    interrupt::free(|cs| {
        let threads = unsafe { Threads::get_mut(cs) };
        let pid = threads.create(func, arg, stack, prio).unwrap().pid;
        threads.set_state(pid, ThreadState::Running);
    });
}

/// thread cleanup function
///
/// This gets hooked into a newly created thread stack so it gets called when
/// the thread function returns.
fn cleanup() -> ! {
    let pid = interrupt::free(|cs| {
        let threads = unsafe { Threads::get_mut(cs) };
        let pid = threads.current_pid().unwrap();
        threads.set_state(pid, ThreadState::Invalid);
        pid
    });

    println!("thread {}: exited", pid);

    arch::schedule();

    unreachable!();
}

#[naked]
#[no_mangle]
#[allow(non_snake_case)]
unsafe extern "C" fn SVCall() {
    asm!(
        "
            movw LR, #0xFFFd
            movt LR, #0xFFFF
            bx lr
            ",
        options(noreturn)
    );
}

#[naked]
#[no_mangle]
#[allow(non_snake_case)]
unsafe extern "C" fn PendSV() {
    asm!(
        "
            mrs r0, psp
            cpsid i
            bl {sched}
            cpsie i
            cmp r0, #0
            /* label rules:
             * - number only
             * - no combination of *only* [01]
             * - add f or b for 'next matching forward/backward'
             * so let's use '99' forward ('99f')
             */
            beq 99f
            stmia r0, {{r4-r11}}
            ldmia r1, {{r4-r11}}
            msr.n psp, r2
            99:
            movw LR, #0xFFFd
            movt LR, #0xFFFF
            bx LR
            ",
        sched = sym sched,
        options(noreturn)
    );
}
