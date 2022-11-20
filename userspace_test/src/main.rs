#![no_std]
#![no_main]

use core::panic::PanicInfo;

fn _start() -> ! {
    x86_64::instructions::interrupts::int3();
    panic!();
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}
