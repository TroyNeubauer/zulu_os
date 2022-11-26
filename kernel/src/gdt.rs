use crate::println;
use alloc::boxed::Box;
use core::arch::asm;
use core::num::NonZeroU64;
use memoffset::offset_of;
use num_enum::TryFromPrimitive;
use x86_64::instructions::segmentation::{CS, DS, GS};
use x86_64::registers::rflags::RFlags;
use x86_64::registers::segmentation::Segment64;
use x86_64::structures::gdt::{Descriptor, GlobalDescriptorTable, SegmentSelector};
use x86_64::structures::paging::{Page, Size4KiB, Translate};
use x86_64::structures::tss::TaskStateSegment;
use x86_64::VirtAddr;

pub const DOUBLE_FAULT_STACK_INDEX: u16 = 0;
pub const PAGE_FAULT_STACK_INDEX: u16 = 1;

lazy_static::lazy_static! {
    static ref TSS: TaskStateSegment = {
        let mut tss = TaskStateSegment::new();
        tss.interrupt_stack_table[DOUBLE_FAULT_STACK_INDEX as usize] = {
            const STACK_SIZE: usize = 1024 * 20;
            static mut STACK: [u8; STACK_SIZE] = [0; STACK_SIZE];

            let stack_start = VirtAddr::from_ptr(unsafe { &STACK });
            stack_start + STACK_SIZE
        };

        tss.interrupt_stack_table[PAGE_FAULT_STACK_INDEX as usize] = {
            const STACK_SIZE: usize = 1024 * 20;
            static mut STACK: [u8; STACK_SIZE] = [0; STACK_SIZE];

            let stack_start = VirtAddr::from_ptr(unsafe { &STACK });
            stack_start + STACK_SIZE
        };

        tss.privilege_stack_table[0] = {
            const STACK_SIZE: usize = 1024 * 20;
            static mut STACK: [u8; STACK_SIZE] = [0; STACK_SIZE];

            let stack_start = VirtAddr::from_ptr(unsafe { &STACK });
            stack_start + STACK_SIZE
        };

        tss
    };
}

lazy_static::lazy_static! {
    static ref GDT: (GlobalDescriptorTable, Selectors) = {
        let mut gdt = GlobalDescriptorTable::new();

        let kernel_code_selector = gdt.add_entry(Descriptor::kernel_code_segment());
        let kernel_data_selector = gdt.add_entry(Descriptor::kernel_data_segment());

        let user_data_selector = gdt.add_entry(Descriptor::user_data_segment());
        let user_code_selector = gdt.add_entry(Descriptor::user_code_segment());

        let tss_selector = gdt.add_entry(Descriptor::tss_segment(&TSS));
        let kernel_data_selector2 = gdt.add_entry(Descriptor::kernel_data_segment());

        let selectors = Selectors {
            kernel_code_selector,
            kernel_data_selector,
            user_code_selector,
            user_data_selector,
            tss_selector,
            kernel_data_selector2,
        };

        (gdt, selectors)
    };
}

struct Selectors {
    kernel_code_selector: SegmentSelector,
    kernel_data_selector: SegmentSelector,
    user_code_selector: SegmentSelector,
    user_data_selector: SegmentSelector,
    tss_selector: SegmentSelector,
    kernel_data_selector2: SegmentSelector,
}

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
        let syscall: Syscall = syscall_num.try_into().map_err(|_| Error::NoSys)?;

        match syscall {
            Syscall::Read => Err(Error::NoSys),
            Syscall::Write => with_user_slice(arg1, arg2, |bytes| syscalls::write(arg0, bytes))?,
            Syscall::Exit => {
                crate::sys::enable_interrupts();
                // End of process on this core so this will never return
                // TODO: Call into the scheduler here
                crate::sys::hlt_loop();
            }
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

mod syscalls {
    use super::Result;

    pub fn write(_fd: usize, bytes: &[u8]) -> Result<usize> {
        crate::vga_buffer::print_bytes(bytes);
        Ok(bytes.len())
    }
}

/// Ensures that a user pointer is properly mapped
fn check_user_addr(addr: VirtAddr) -> Result<()> {
    // SAFETY:
    // 1. This will never be called recursively because `check_user_page` has no fn arguments
    // 2. This is a private method that can only be invoked in a syscall, after memory::init has been called
    unsafe { crate::memory::mapper() }.with(|mapper| match mapper.translate_addr(addr) {
        Some(_) => Ok(()),
        None => Err(Error::InvalidArgument),
    })
}

// TODO: should this be unsafe??
fn with_user_slice<F, R>(ptr: usize, bytes: usize, f: F) -> Result<R>
where
    F: FnOnce(&[u8]) -> R,
{
    // SAFETY: `t` is limited to the lifetime of the closure `f`, so there is no time for the bytes
    // within the closure to be invalidated. The user would have to return the address or modify a
    // global, all of which require additional unsafe code to break memory safety
    unsafe { construct_user_slice(ptr, bytes) }.map(f)
}

/// Creates a rust slice to a user pointer array after verifying that the memory is mapped
///
/// # Safety:
/// The caller must guarntee that `ptr` is valid for the lifetime they choose `'t`
unsafe fn construct_user_slice<'t>(ptr: usize, bytes: usize) -> Result<&'t [u8]> {
    let addr = VirtAddr::try_new(ptr as u64).map_err(|_| Error::InvalidArgument)?;
    if bytes > isize::MAX as usize {
        return Err(Error::InvalidArgument);
    }

    let first_page = Page::<Size4KiB>::containing_address(addr);
    let last_page = Page::containing_address(addr + (bytes - 1));
    for page in Page::range_inclusive(first_page, last_page) {
        check_user_addr(page.start_address())?;
    }

    let data = ptr as *const u8;
    // SAFETY:
    // 1. `data` is valid for reads of len bytes because all pages are mapped by the logic above
    //    a. We have no way of proving that the entire range is within the same allocated object,
    //       but as this is kernel code, it should be okay to access any of these bytes
    //    b. `data` is guanrteed to be non-null because we never map the null page and we know `data`
    //       points to a mapped page by the check above
    // 2. `data` is guarnteed to be aligned because it is a u8, and it is proven to be mapped,
    //    therefore it is initialized. TODO This doesn't seem sufficent???
    // 3. The caller guarntees that ptr is valid for `'t`
    // 4. We check above that `bytes` is no larger than `isize::MAX`
    Ok(unsafe { core::slice::from_raw_parts(data, bytes) })
}

#[naked]
#[no_mangle]
extern "x86-interrupt" fn syscall_handler() {
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

#[derive(Clone, Debug)]
#[repr(C)]
pub struct ThreadData {
    /// RSP of the kernel stack (or user stack if handling syscall)
    pub kernel_rsp: Option<NonZeroU64>,
    /// RSP of the user task that initialized this syscall
    pub user_tmp_rsp: Option<NonZeroU64>,
}

/// Gets a mutable reference to this therad's kernel data structure in GS
///
/// # Safety
/// 1. The caller must guarntee that this function is never called on the same thread if there is an
///    active ThreadData reference (mutable aliasing is UB), including if a thread is interrupted
///    while in a syscall
/// 2. This function must not be called before `gdt_init` is called
///
/// NOTE: Interrupts are disabled for the duration that `f` runs to prevent data races, so the
/// critical section of `f` should be short. This also means that `with_thread_data` can be
/// accessed during interrupts to obtain access to kernel data
pub unsafe fn with_thread_data<F, R>(f: F) -> R
where
    F: FnOnce(&mut ThreadData) -> R,
{
    // disable interrupts to prevent easy mutable aliasing because the interrupt handler calls this
    crate::sys::without_interrupts(|| {
        let gs = GS::read_base();
        debug_assert!(!gs.is_null());
        // SAFETY: By the contract above, gs is initialized and we have exclusive access
        f(unsafe { &mut *gs.as_mut_ptr() })
    })
}

pub fn init_thread_data(data: ThreadData) {
    let thread_data: *mut ThreadData = Box::into_raw(Box::new(data));
    let thread_data = VirtAddr::new(thread_data as usize as u64);
    unsafe { GS::write_base(thread_data) };
}

#[no_mangle]
pub fn gdt_init() {
    use x86_64::instructions::tables::load_tss;
    use x86_64::registers::control::{Cr4, Cr4Flags};
    use x86_64::registers::model_specific::{Efer, EferFlags, LStar, SFMask, Star};
    use x86_64::registers::segmentation::Segment;

    use raw_cpuid::CpuId;
    let cpuid = CpuId::new();

    let has_fsgbase = cpuid
        .get_extended_feature_info()
        .map_or(false, |info| info.has_fsgsbase());
    assert!(has_fsgbase);

    GDT.0.load();
    let syscall_rip = VirtAddr::new(syscall_handler as usize as u64);
    unsafe {
        CS::set_reg(GDT.1.kernel_code_selector);
        DS::set_reg(GDT.1.kernel_data_selector);
        GS::set_reg(GDT.1.kernel_data_selector2);

        Star::write(
            GDT.1.user_code_selector,
            GDT.1.user_data_selector,
            GDT.1.kernel_code_selector,
            GDT.1.kernel_data_selector,
        )
        .unwrap();

        Efer::update(|f| f.set(EferFlags::SYSTEM_CALL_EXTENSIONS, true));
        // enable storing kernel defined pointers inside of FS and GS
        Cr4::update(|c| c.set(Cr4Flags::FSGSBASE, true));

        // Interrupts are always disabled for the duration of syscalls. This allows us to have only
        // one kernel stack
        let flags_to_clear = SFMask::read() | RFlags::INTERRUPT_FLAG;
        SFMask::write(flags_to_clear);

        LStar::write(syscall_rip);
        load_tss(GDT.1.tss_selector);
    }
}

#[cfg(test)]
mod tests {
    use core::mem::MaybeUninit;

    #[test_case]
    fn thread_data_alignment() {
        use super::*;
        use core::mem::{size_of, size_of_val, transmute};
        let data = ThreadData {
            kernel_rsp: None,
            kernel_rbp: None,
        };
        // We depend on `Option<NonZeroU64>` having the niche optimization, (None == 0)
        // so that we can write to this in assembly and still have correctness
        assert_eq!(size_of_val(&data.kernel_rsp), size_of::<u64>());
        assert_eq!(
            unsafe { MaybeUninit::<Option<NonZeroU64>>::zeroed().assume_init() },
            None
        );
        // we depend on this in syscall handler
        assert_eq!(offset_of!(ThreadData, kernel_rsp), 0);
        assert_eq!(offset_of!(ThreadData, kernel_rbp), 8);

        assert_eq!(
            unsafe { transmute::<_, Option<NonZeroU64>>(10u64) },
            Some(NonZeroU64::new(10).unwrap())
        );
    }
}
