#![no_std]
#![no_main]
#![feature(naked_functions)]

#[no_mangle]
pub extern "C" fn _start() -> ! {
    let s = "Test print";
    unsafe { syscall::syscall_3(2, 0, 0, 0) };
    // write (stdout, string, len)
    syscall::write(0, s.as_bytes());

    // exit (code 0)
    syscall::exit(0);
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    syscall::exit(1);
}
