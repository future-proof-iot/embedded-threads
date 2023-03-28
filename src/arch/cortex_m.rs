use core::arch::asm;
use core::ptr::write_volatile;
pub use cortex_m::interrupt::{self, CriticalSection, Mutex};
use cortex_m::peripheral::SCB;

use crate::{cleanup, sched};

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
