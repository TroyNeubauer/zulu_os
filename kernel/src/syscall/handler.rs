use super::{io, with_user_slice, with_user_slice_mut, ThreadData};
use crate::println;
use core::arch::asm;
use memoffset::offset_of;
use syscall::{Error, Result, Syscall};

#[no_mangle]
extern "sysv64" fn syscall_handler_inner(
    syscall_num: usize,
    arg0: usize,
    arg1: usize,
    arg2: usize,
    arg3: usize,
    arg4: usize,
) -> usize {
    let inner = || -> Result<usize> {
        println!(
        "SYSCALL: num: {syscall_num}, 0: 0x{arg0:X}, 1: 0x{arg1:X} , 2: 0x{arg2:X} , 3: 0x{arg3:X} , 4: 0x{arg4:X}",
    );
        let syscall_num: u8 = syscall_num.try_into().map_err(|_| Error::NoSys)?;
        println!("made u8 syscall num ok");
        let syscall: Syscall = syscall_num.try_into().map_err(|_| Error::NoSys)?;
        println!("parsed syscall num ok");

        match syscall {
            // SAFETY: If the slice can be constructed, then it means it is a user page which we do
            // not alias in the kernel. This memory may be aliased in the user program, but they
            // are paused so they cannot observe that we alias the same memory here
            Syscall::Read => unsafe {
                with_user_slice_mut(arg1, arg2, |bytes| io::read(arg0, bytes))?
            },
            Syscall::Write => with_user_slice(arg1, arg2, |bytes| io::write(arg0, bytes))?,
            Syscall::Exit => super::process::exit(arg0 as u8),
        }
    };

    match inner() {
        Ok(val) => {
            let small: isize = val
                .try_into()
                .expect("kernel returned too large return value");
            small as usize
        }
        Err(e) => {
            // Make error code a negitive number
            let neg_err = -(e as u8 as isize);
            // convert back to usize for return
            neg_err as usize
        }
    }
}

#[naked]
#[no_mangle]
pub(super) extern "x86-interrupt" fn syscall_handler() {
    // rbx, rsp, rbp, and r12–r15, need to be preserved inside syscall
    unsafe {
        asm!(
            // Swap user stack with kernel stack using rax as temp register
            // Swap rsp
            "swapgs",
            "mov gs:[{user_rsp_offset}], rsp",
            "mov rsp, gs:[{kernel_rsp_offset}]",
            //
            // Registers on entry:
            //
            // rcx  return address
            // r11  saved rflags
            // rdi  system call number
            // rsi  arg0
            // rdx  arg1
            // r10  arg2
            // r8   arg3
            // r9   arg4
            //
            // Push return address
            "push rcx",
            // Push saved rflags
            "push r11",
            // SystemV expects registers in the folowing order for calling syscall_handler_inner
            //
            // rdi  syscall number
            // rsi  arg0
            // rdx  arg1
            // rcx  arg2 => gets set by syscall to return pointer (needs replacing)
            // r8   arg3
            // r9   arg4
            "mov rcx, r10",
            "call syscall_handler_inner",
            // rax now holds return value from call
            // pop saved flags
            "pop r11",
            // pop saved return address
            "pop rcx",
            // Restore user stack
            "mov gs:[{kernel_rsp_offset}], rsp",
            "mov rsp, gs:[{user_rsp_offset}]",
            "swapgs",
            "sysretq",
            kernel_rsp_offset = const(offset_of!(ThreadData, kernel_rsp)),
            user_rsp_offset = const(offset_of!(ThreadData, user_tmp_rsp)),
            options(noreturn)
        )
    };
}
