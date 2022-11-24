#![no_std]
#![no_main]
#![feature(naked_functions)]

use core::{arch::asm, panic::PanicInfo};

#[no_mangle]
pub extern "C" fn _start() -> ! {
    let s = "Test print";
    // write (stdout, string, len)
    unsafe { syscall_3(2, 0, s.as_ptr() as usize, s.len()) };

    // exit (code 0)
    unsafe { syscall_1(3, 0) };
    loop {}
}

macro_rules! syscall {
    (
        $name:ident(
            $($arg0:ident, // rsi
                $($arg1:ident, // rdx
                    $($arg2:ident, // rcx
                        $($arg3:ident, // r8
                            $($arg4:ident,)? // r9
                        )?
                    )?
                )?
            )?
        )
    ) => {
        #[naked]
        pub unsafe extern "sysv64" fn $name(
            syscall_num: usize,
            $($arg0: usize,
                $($arg1: usize,
                    $($arg2: usize,
                        $($arg3: usize,
                            $($arg4: usize)?
                        )?
                    )?
                )?
            )?
        ) -> usize {
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
                    // setup normal stack frame to make debugging userspace more useful
                    "push rbp",
                    "mov rbp, rsp",
                    // we clobber r10 and r11, but these do not need to be preserved for the caller
                    // rsi (arg0) already in place
                    // rdx (arg1) already in place
                    "mov r10, rcx", // put arg2 in place
                    // r8 (arg3) already in place
                    // r9 (arg4) already in place
                    "syscall",
                    // restore stack
                    "leave",
                    "ret",
                    options(noreturn)
                )
            }
        }
    }
}

syscall!(syscall_0());
syscall!(syscall_1(arg0,));
syscall!(syscall_2(arg0, arg1,));
syscall!(syscall_3(arg0, arg1, arg2,));
syscall!(syscall_4(arg0, arg1, arg2, arg3,));
syscall!(syscall_5(arg0, arg1, arg2, arg3, arg4,));

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}
