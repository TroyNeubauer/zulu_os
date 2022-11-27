pub mod handler;
pub mod io;
pub mod process;

use alloc::boxed::Box;
use core::num::NonZeroU64;
use syscall::{Error, Result};
use x86_64::instructions::segmentation::GS;
use x86_64::registers::control::{Cr4, Cr4Flags};
use x86_64::registers::model_specific::{Efer, EferFlags, LStar, SFMask};
use x86_64::registers::rflags::RFlags;
use x86_64::registers::segmentation::Segment64;
use x86_64::structures::paging::mapper::TranslateResult;
use x86_64::structures::paging::{Page, PageTableFlags, Size4KiB, Translate};
use x86_64::VirtAddr;

pub fn init() {
    let syscall_rip = VirtAddr::new(handler::syscall_handler as usize as u64);
    // Interrupts are always disabled for the duration of syscalls. This allows us to have only
    // one kernel stack
    let flags_to_clear = SFMask::read() | RFlags::INTERRUPT_FLAG;
    SFMask::write(flags_to_clear);

    unsafe { Efer::update(|f| f.set(EferFlags::SYSTEM_CALL_EXTENSIONS, true)) };
    // enable storing kernel defined pointers inside of FS and GS
    unsafe { Cr4::update(|c| c.set(Cr4Flags::FSGSBASE, true)) };

    LStar::write(syscall_rip);
}

enum ReadAccess {
    ReadOnly,
    ReadWrite,
}

/// Ensures that a user pointer is properly mapped, and that the page is writable if `writable` is
/// true
fn check_user_addr(addr: VirtAddr, access: ReadAccess) -> Result<()> {
    // SAFETY:
    // 1. This will never be called recursively because `check_user_page` has no fn arguments
    // 2. This is a private method that can only be invoked by a syscall, after memory::init has been called
    unsafe { crate::memory::mapper() }.with(|mapper| match mapper.translate(addr) {
        TranslateResult::NotMapped => Err(Error::InvalidArgument),
        TranslateResult::InvalidFrameAddress(_) => Err(Error::InvalidArgument),
        TranslateResult::Mapped { flags, .. } => {
            if !flags.contains(PageTableFlags::USER_ACCESSIBLE) {
                return Err(Error::InvalidArgument);
            }
            match access {
                ReadAccess::ReadOnly => Ok(()),
                ReadAccess::ReadWrite => {
                    if flags.contains(PageTableFlags::WRITABLE) {
                        Ok(())
                    } else {
                        return Err(Error::InvalidArgument);
                    }
                }
            }
        }
    })
}

// TODO: should this be unsafe?? `ptr` is guarnteed to be a user acessible here, so unless we
// accidentally allocated a kernel address that was user acessible, we know `ptr` is mapped
// and user executition is paused so there are no observable effects of creating this reference
fn with_user_slice<F, R>(ptr: usize, bytes: usize, f: F) -> Result<R>
where
    F: FnOnce(&[u8]) -> R,
{
    // SAFETY: `t` is limited to the lifetime of the closure `f`, so there is no time for the bytes
    // within the closure to be invalidated. The caller would have to return the address or modify a
    // global, all of which require additional unsafe code to break memory safety
    unsafe { construct_user_slice(ptr, bytes) }.map(f)
}

/// # Safety
/// The caller must guarntee that the range `ptr` to `ptr + bytes` is not aliased if a slice can be
/// constructed (the memory range is mapped and user acessible)
unsafe fn with_user_slice_mut<F, R>(ptr: usize, bytes: usize, f: F) -> Result<R>
where
    F: FnOnce(&mut [u8]) -> R,
{
    // SAFETY: `t` is limited to the lifetime of the closure `f`, so there is no time for the bytes
    // within the closure to be invalidated. The caller would have to return the address or modify a
    // global, all of which require additional unsafe code to break memory safety
    unsafe { construct_user_slice_mut(ptr, bytes) }.map(f)
}

/// Creates a rust slice to a user pointer array after verifying that the memory is mapped
///
/// # Safety:
///
/// The caller must guarntee that `ptr` is valid for the lifetime they choose `'t`
unsafe fn construct_user_slice<'t>(ptr: usize, bytes: usize) -> Result<&'t [u8]> {
    let addr = VirtAddr::try_new(ptr as u64).map_err(|_| Error::InvalidArgument)?;
    if bytes > isize::MAX as usize {
        return Err(Error::InvalidArgument);
    }

    let first_page = Page::<Size4KiB>::containing_address(addr);
    let last_page = Page::containing_address(addr + (bytes - 1));
    for page in Page::range_inclusive(first_page, last_page) {
        check_user_addr(page.start_address(), ReadAccess::ReadOnly)?;
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

/// Creates a mutable rust slice to a user pointer array after verifying that the memory is mapped,
/// writable, and user acessible
///
/// # Safety:
///
/// 1. The caller must guarntee that `ptr` is valid for the lifetime they choose `'t`
/// 2. The caller must guarntee that the range `ptr` to `ptr + bytes` is not aliased if a slice
/// can be constructed (the memory range is mapped and user acessible)
unsafe fn construct_user_slice_mut<'t>(ptr: usize, bytes: usize) -> Result<&'t mut [u8]> {
    let addr = VirtAddr::try_new(ptr as u64).map_err(|_| Error::InvalidArgument)?;
    if bytes > isize::MAX as usize {
        return Err(Error::InvalidArgument);
    }

    let first_page = Page::<Size4KiB>::containing_address(addr);
    let last_page = Page::containing_address(addr + (bytes - 1));
    for page in Page::range_inclusive(first_page, last_page) {
        check_user_addr(page.start_address(), ReadAccess::ReadWrite)?;
    }

    let data = ptr as *mut u8;
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
    Ok(unsafe { core::slice::from_raw_parts_mut(data, bytes) })
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

#[cfg(test)]
mod tests {
    use core::mem::MaybeUninit;
    use memoffset::offset_of;

    #[test_case]
    fn thread_data_alignment() {
        use super::*;
        use core::mem::{size_of, size_of_val, transmute};
        let data = ThreadData {
            kernel_rsp: None,
            user_tmp_rsp: None,
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
        assert_eq!(offset_of!(ThreadData, user_tmp_rsp), 8);

        assert_eq!(
            unsafe { transmute::<_, Option<NonZeroU64>>(10u64) },
            Some(NonZeroU64::new(10).unwrap())
        );
    }
}
