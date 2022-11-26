#![no_std]
#![feature(naked_functions)]
#![feature(core_intrinsics)]

use core::arch::asm;
use core::hint::unreachable_unchecked;
use num_enum::TryFromPrimitive;

#[derive(Copy, Clone, Debug, TryFromPrimitive)]
#[repr(u8)]
pub enum Syscall {
    Read = 1,
    Write = 2,
    Exit = 3,
}

#[derive(Copy, Clone, Debug, TryFromPrimitive)]
#[repr(u8)]
pub enum Error {
    /// No such system call
    NoSys,
    /// Invalid argument to syscall
    InvalidArgument,
}

pub type Result<T> = core::result::Result<T, Error>;

#[inline]
pub fn write(fd: u32, bytes: &[u8]) -> usize {
    unsafe {
        syscall_3(
            Syscall::Write as usize,
            fd as usize,
            bytes.as_ptr() as usize,
            bytes.len(),
        )
    }
}

#[inline]
pub fn read(fd: u32, bytes: &mut [u8]) -> usize {
    unsafe {
        syscall_3(
            Syscall::Read as usize,
            fd as usize,
            bytes.as_mut_ptr() as usize,
            bytes.len(),
        )
    }
}

#[inline]
pub fn exit(code: u32) -> ! {
    unsafe { syscall_1(Syscall::Exit as usize, code as usize) };
    unsafe { unreachable_unchecked() };
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
