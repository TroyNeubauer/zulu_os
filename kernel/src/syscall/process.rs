use syscall::Result;

pub fn exit(code: u8) -> Result<usize> {
    crate::sys::enable_interrupts();
    // End of process on this core so this will never return
    // TODO: Call into the scheduler here
    crate::sys::hlt_loop();
}
