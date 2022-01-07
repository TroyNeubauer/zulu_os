
pub fn without_interrupts<F>(f: F)
where
    F: FnOnce(),
{
    x86_64::instructions::interrupts::without_interrupts(f);
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
