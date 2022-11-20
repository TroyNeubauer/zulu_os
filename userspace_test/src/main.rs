#![no_std]
#![no_main]
#![feature(naked_functions)]

use core::{
    arch::asm,
    panic::PanicInfo,
    ptr,
    sync::atomic::AtomicIsize,
    sync::atomic::{compiler_fence, AtomicUsize, Ordering},
};

pub static VAL: AtomicUsize = AtomicUsize::new(0);

#[no_mangle]
pub extern "C" fn _start() -> ! {
    //print("before syscall");
    syscall();
    print("after syscall");
    loop {}
}

fn syscall() {
    compiler_fence(Ordering::SeqCst);
    syscall_inner();
}

#[naked]
extern "C" fn syscall_inner() {
    unsafe { asm!("int3", "ret", options(noreturn),) }
}

fn print(s: &str) {
    let vga_buffer = 0xb8000 as *mut u8;
    static OFFSET: AtomicIsize = AtomicIsize::new(0);
    let offset = OFFSET.fetch_add(s.len() as isize, Ordering::SeqCst);

    for (i, byte) in s.bytes().enumerate() {
        let pos = i as isize * 2 + offset;
        unsafe {
            ptr::write_volatile(vga_buffer.offset(pos), byte);
            ptr::write_volatile(vga_buffer.offset(pos + 1), 0xb);
        }
    }
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}
