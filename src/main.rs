#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(zulu_os::test_runner)]
#![reexport_test_harness_main = "test_main"]

use core::panic::PanicInfo;
use zulu_os::{print, println};

#[no_mangle]
pub extern "C" fn _start() -> ! {
    zulu_os::init();

    #[cfg(test)]
    test_main();

    zulu_os::sys::hlt_loop();
}

/// This function is called on panic.
#[cfg(not(test))]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("{}", info);
    zulu_os::sys::hlt_loop();
}

#[cfg(test)]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    zulu_os::test_panic_handler(info)
}
