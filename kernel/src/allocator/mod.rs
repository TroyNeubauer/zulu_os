pub mod bump;

pub use bump::*;

use x86_64::structures::paging::mapper::MapToError;
use x86_64::structures::paging::{FrameAllocator, Mapper, Page, PageTableFlags, Size4KiB};
use x86_64::VirtAddr;

pub const HEAP_START: usize = 0x_4444_4444_0000;
pub const HEAP_SIZE: usize = 100 * 1024;

/// Initializes the kernel heap by allocating same pages and then initializing the allocator
///
/// # Safety
/// 1. The caller must ensure that this function is only called once
/// 2. The caller must not allocate any objects before this function returns
pub unsafe fn init_kernel_heap(
    mapper: &mut impl Mapper<Size4KiB>,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
) -> Result<(), MapToError<Size4KiB>> {
    let page_range = {
        let heap_start = VirtAddr::new(HEAP_START as u64);
        let heap_end = heap_start + HEAP_SIZE - 1u64;
        let heap_start_page = Page::containing_address(heap_start);
        let heap_end_page = Page::containing_address(heap_end);

        Page::range_inclusive(heap_start_page, heap_end_page)
    };

    for page in page_range {
        let frame = frame_allocator
            .allocate_frame()
            .ok_or(MapToError::FrameAllocationFailed)?;

        let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE;
        unsafe {
            mapper.map_to(page, frame, flags, frame_allocator)?.flush();
        };
        //TODO: optimize here
    }

    // # Safety: The caller has ensured that this function is only called once
    unsafe { ALLOCATOR.init(HEAP_START, HEAP_SIZE) };
    Ok(())
}

#[global_allocator]
static ALLOCATOR: BumpAllocator = BumpAllocator::new();

/// Align the given value `value` upwards to alignment `align`.
///
/// #Safety
/// `align` must be a power of two.
unsafe fn align_up(value: usize, align: usize) -> usize {
    (value + align - 1) & !(align - 1)
}
