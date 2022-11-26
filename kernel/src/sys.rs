pub fn without_interrupts<F, R>(f: F) -> R
where
    F: FnOnce() -> R,
{
    x86_64::instructions::interrupts::without_interrupts(f)
}

/// Runs `f` in an interrupt free context and waits for an interrupt if `true` is returned.
/// If `false` is returned, interrupts are re-enabled and life continues as normal
pub fn wait_for_interrupts_if<F>(f: F)
where
    F: FnOnce() -> bool,
{
    x86_64::instructions::interrupts::disable();
    if f() {
        x86_64::instructions::interrupts::enable_and_hlt();
    } else {
        x86_64::instructions::interrupts::enable();
    }
}

#[inline]
pub fn hlt() {
    x86_64::instructions::hlt();
}

pub fn hlt_loop() -> ! {
    loop {
        hlt();
    }
}

#[inline]
pub fn enable_interrupts() {
    x86_64::instructions::interrupts::enable();
}

#[inline]
pub fn disable_interrupts() {
    x86_64::instructions::interrupts::disable();
}
