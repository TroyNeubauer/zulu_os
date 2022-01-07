#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(zulu_os::test_runner)]
#![reexport_test_harness_main = "test_main"]

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    zulu_os::test_panic_handler(info)
}

use core::panic::PanicInfo;

#[no_mangle] // don't mangle the name of this function
pub extern "C" fn _start() -> ! {
    test_main();

    loop {}
}

use zulu_os::println;

#[test_case]
fn test_println() {
    println!("test_println output");
}
