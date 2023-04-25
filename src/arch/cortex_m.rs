use core::arch::asm;
use core::ptr::write_volatile;
use cortex_m::peripheral::SCB;
use critical_section::CriticalSection;

use crate::{cleanup, THREADS};

/// Sets up stack for newly created threads.
///
/// After running this, the stack should look as if the thread was
/// interrupted by an ISR. On the next return, it starts executing
/// `func`.
pub(crate) fn setup_stack(stack: &mut [u8], func: usize, arg: usize) -> usize {
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

#[inline(always)]
pub fn schedule() {
    SCB::set_pendsv();
    cortex_m::asm::isb();
}

#[inline(always)]
pub(crate) fn start_threading(next_sp: usize) {
    unsafe {
        asm!(
            "
            msr psp, r1 // set new thread's SP to PSP
            cpsie i     // enable interrupts, otherwise svc hard faults
            svc 0       // SVC 0 handles switching
            ",
        in("r1")next_sp);
    }
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

/// scheduler
///
/// On Cortex-M, this is called in PendSV.
// TODO: make arch independent, or move to arch
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
