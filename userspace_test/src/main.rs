#![no_std]
#![no_main]

use core::panic::PanicInfo;

fn _start() -> ! {
    panic!();
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}
