#![no_std]
#![no_main]

use core::{
    panic::PanicInfo,
    sync::atomic::{AtomicUsize, Ordering},
};

pub static VAL: AtomicUsize = AtomicUsize::new(0);

#[no_mangle]
pub extern "C" fn _start() -> ! {
    VAL.fetch_add(1, Ordering::SeqCst);
    x86_64::instructions::interrupts::int3();
    x86_64::instructions::interrupts::int3();
    x86_64::instructions::interrupts::int3();
    x86_64::instructions::interrupts::int3();
    VAL.fetch_add(2, Ordering::SeqCst);
    panic!();
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    loop {
        let a = unsafe { core::ptr::read_volatile(info as *const _) };
        let b = unsafe { core::ptr::read_volatile(&VAL as *const _) };
        x86_64::instructions::interrupts::int3();
    }
}
