#![no_std]
#![no_main]
#![feature(naked_functions)]

use core::{
    arch::asm,
    panic::PanicInfo,
    ptr,
    sync::atomic::{AtomicIsize, AtomicUsize, Ordering},
};

pub static VAL: AtomicUsize = AtomicUsize::new(0);

#[no_mangle]
pub extern "C" fn _start() -> ! {
    // let s = "TEST\nABC";
    syscall(0x6969, 0, 1, 2, 3, 4);
    print("child didnt crash!");
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
            // rsi (arg0) already in place
            // rdx (arg1) already in place
            "mov r10, rcx", // put arg2 in place
            // r8 (arg3) already in place
            // r9 (arg4) already in place
            "syscall",
            "ret",
            options(noreturn)
        )
    }
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

macro_rules! syscall {
    ($($name:ident($a:ident, $($b:ident, $($c:ident, $($d:ident, $($e:ident, $($f:ident, )?)?)?)?)?);)+) => {
        $(
            pub unsafe fn $name(mut $a: usize, $($b: usize, $($c: usize, $($d: usize, $($e: usize, $($f: usize)?)?)?)?)?) -> usize {
                asm!(
                    "syscall",
                    inout("rax") $a,
                    $(
                        in("rdi") $b,
                        $(
                            in("rsi") $c,
                            $(
                                in("rdx") $d,
                                $(
                                    in("r10") $e,
                                    $(
                                        in("r8") $f,
                                    )?
                                )?
                            )?
                        )?
                    )?
                    out("rcx") _,
                    out("r11") _,
                    options(nostack),
                );

                $a
            }
        )+
    };
}

syscall! {
    syscall0(a,);
    syscall1(a, b,);
    syscall2(a, b, c,);
    syscall3(a, b, c, d,);
    syscall4(a, b, c, d, e,);
    syscall5(a, b, c, d, e, f,);
}
