#![no_std]
#![no_main]
#![feature(naked_functions)]

use core::{
    arch::asm,
    panic::PanicInfo,
    ptr,
    sync::atomic::{compiler_fence, AtomicIsize, AtomicUsize, Ordering},
};

pub static VAL: AtomicUsize = AtomicUsize::new(0);

#[no_mangle]
pub extern "C" fn _start() -> ! {
    let s = "TEST\nABC";
    syscall(s.as_ptr(), s.len());
    print("child didnt crash!");
    loop {}
}

fn syscall(ptr: *const u8, len: usize) {
    compiler_fence(Ordering::SeqCst);
    unsafe {
        asm!(
            "mov rdi, {ptr}",
            "mov rsi, {len}",
            "syscall",
            ptr = in(reg) ptr,
            len = in(reg) len,
            out("rdi") _,
            out("rsi") _,
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
