use core::arch::asm;
pub use cortex_m::interrupt::{self, CriticalSection, Mutex};
use cortex_m::peripheral::SCB;

use crate::sched;

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
