#![no_std]
#![no_main]
// used by Kernel::threads initializer, ...
#![feature(inline_const)]
// used by ISRs
#![feature(naked_functions)]
// used by PendSV to get `sched`
#![feature(asm_sym)]

use cortex_m::interrupt;
use cortex_m_rt::entry;
use cortex_m_semihosting::{
    debug::{self, EXIT_SUCCESS},
    hprintln as println,
};

use panic_semihosting as _;

use crate::threads::Threads;

mod threads;

static mut STACK: [u8; 2048] = [0; 2048];
static mut STACK2: [u8; 2048] = [0; 2048];

fn test_thread(arg: usize) {
    println!("test_thread() arg={}", arg);

    // testing asserts
    assert!(1 == 1);
}

#[entry]
fn main() -> ! {
    println!("main() creating thread");
    interrupt::free(|cs| {
        let threads = unsafe { threads::Threads::get_mut(&cs) };
        let pid = threads
            .create(unsafe { &mut STACK }, test_thread, 0, 0)
            .unwrap()
            .pid;
        threads.set_state(pid, threads::ThreadState::Running);
    });

    println!("main() creating thread 2");
    interrupt::free(|cs| {
        let threads = unsafe { threads::Threads::get_mut(&cs) };
        let pid = threads
            .create(unsafe { &mut STACK2 }, test_thread, 1, 1)
            .unwrap()
            .pid;
        println!("thread 2 pid={}", pid);
        threads.set_state(pid, threads::ThreadState::Running);
    });

    println!("main() post thread create");

    unsafe { Threads::start_threading() };

    println!("main() shouldn't be here");
    // exit via semihosting call
    debug::exit(EXIT_SUCCESS);

    // the cortex_m_rt `entry` macro requires `main()` to never return
    loop {}
}
