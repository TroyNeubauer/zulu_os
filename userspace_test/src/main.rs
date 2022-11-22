#![no_std]
#![no_main]
#![feature(naked_functions)]

use core::{
    arch::asm,
    panic::PanicInfo,
    ptr,
    sync::atomic::{AtomicIsize, AtomicUsize, Ordering},
};

pub static VAL: AtomicUsize = AtomicUsize::new(0xFF00);

#[no_mangle]
pub extern "C" fn _start() -> ! {
    let a = VAL.fetch_add(4, Ordering::Relaxed);
    let b = VAL.fetch_add(4, Ordering::Relaxed);
    let c = VAL.fetch_add(4, Ordering::Relaxed);
    let d = VAL.fetch_add(4, Ordering::Relaxed);
    let e = VAL.fetch_add(4, Ordering::Relaxed);
    syscall(0x6969, a, b, c, d, e);
    loop {}
}

#[naked]
extern "sysv64" fn syscall(
    syscall_num: usize, // rdi
    arg0: usize,        // rsi
    arg1: usize,        // rdx
    arg2: usize,        // rcx
    arg3: usize,        // r8
    arg4: usize,        // r9
) {
    // Kernel syscall format we have to match before issuing syscall
    //
    // rcx  return address (written by syscall)
    // r11  saved rflags(written by syscall)
    // rdi  system call number
    // rsi  arg0
    // rdx  arg1
    // r10  arg2
    // r8   arg3
    // r9   arg4
    //
    //
    // SystemV already has registers in the folowing order:
    //
    // rdi  syscall number
    // rsi  arg0
    // rdx  arg1
    // rcx  arg2 => gets set by syscall to return pointer
    // r8   arg3
    // r9   arg4
    unsafe {
        asm!(
            // we clobber r10 and r11, so backup because these are non-volatile registers
            "push r10",
            "push r11",
            // rsi (arg0) already in place
            // rdx (arg1) already in place
            "mov r10, rcx", // put arg2 in place
            // r8 (arg3) already in place
            // r9 (arg4) already in place
            "syscall",
            "pop r11",
            "pop r10",
            "ret",
            options(noreturn)
        )
    }
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}
