pub use cortex_m::interrupt::{self, CriticalSection, Mutex};
use cortex_m::peripheral::SCB;

pub fn schedule() {
    SCB::set_pendsv();
    cortex_m::asm::isb();
}
