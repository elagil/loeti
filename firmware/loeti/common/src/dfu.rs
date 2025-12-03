//! DFU mode interface.
use cortex_m::asm::bootstrap;
use cortex_m::interrupt;

/// Go to DFU mode.
///
/// # Safety
/// Jumps to raw pointer address.
#[allow(unused)]
pub unsafe fn jump() -> ! {
    // FIXME: Move to board implementation?
    const SYSTEM_MEMORY_BASE: u32 = 0x1FFF_0000;

    interrupt::disable();

    // Read the vector table from system memory
    let vt = SYSTEM_MEMORY_BASE as *const u32;

    // First word: initial MSP
    let msp_value = unsafe { core::ptr::read(vt) };
    // Second word: reset handler
    let rv_value = unsafe { core::ptr::read(vt.add(1)) };

    // Cast the values to pointers for bootstrap
    let msp = msp_value as *const u32;
    let rv = rv_value as *const u32;

    unsafe { bootstrap(msp, rv) }
}
