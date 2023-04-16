use core::mem::MaybeUninit;

use {
    bootloader_api::info::{MemoryRegionKind, MemoryRegions},
    x86_64::{
        structures::paging::{
            FrameAllocator, FrameDeallocator, OffsetPageTable, PageTable, PhysFrame, Size4KiB,
        },
        PhysAddr, VirtAddr,
    },
};

/// Initialize a new OffsetPageTable, and calls `f` with the new page mapper.
///
/// Call [`with_mapper`] to obtain an instance to this therad's mapper later.
///
/// # Safety
///
/// 1. The caller must guarantee that the complete physical memory is mapped to virtual memory at
///     the passed `physical_memory_offset`.
///
/// 2. This function must be only called once, and while interrupt are disabled
pub unsafe fn init<'g>(physical_memory_offset: VirtAddr) -> MapperGuard<'g> {
    use x86_64::registers::control::Cr3;
    let (level4_frame, _) = Cr3::read();

    let phys_addr = level4_frame.start_address();
    let virt_addr = physical_memory_offset + phys_addr.as_u64();
    let page_table_ptr: *mut PageTable = virt_addr.as_mut_ptr();

    // SAFETY: Caller has guaranteed that physical memory is mapped at `physical_memory_offset`
    let level_4_table = unsafe { &mut *page_table_ptr };

    let mapper = unsafe { OffsetPageTable::new(level_4_table, physical_memory_offset) };
    // SAFETY: The caller will only call this function once, and before `with_mapper` is called,
    // therefore there no previous state will be lost and there are no data races
    let mapper = unsafe { MAPPER.write(mapper) };
    MapperGuard { inner: mapper }
}

// TODO: make thread local
static mut MAPPER: MaybeUninit<OffsetPageTable> = MaybeUninit::uninit();

/// Gets a mutable reference to this therad's page mapper
///
/// # Safety
/// 1. The caller must guarntee that this function is never called while another MapperGuard object
///    is alive (mutable aliasing is UB).
///    * This includes safety from interrupts, as interrupt handlers may use the mapper, so
///    interrupts must be disabled during the duration mapper is called
/// 2. This function must not be called before [`crate::memory::init`] is called
#[must_use]
pub unsafe fn mapper<'g>() -> MapperGuard<'g> {
    MapperGuard {
        // SAFETY: Given by mapper's safety contract
        inner: unsafe { MAPPER.assume_init_mut() },
    }
}

pub struct MapperGuard<'g> {
    inner: &'g mut OffsetPageTable<'static>,
}

impl<'g> MapperGuard<'g> {
    pub fn with<F, R>(self, f: F) -> R
    where
        F: FnOnce(&'g mut OffsetPageTable<'static>) -> R,
    {
        f(self.inner)
    }
}

/// A FrameAllocator that returns usable frames from the bootloader's memory map.
pub struct BootInfoFrameAllocator {
    regions: &'static MemoryRegions,
    next: usize,
    last_phys_addr: PhysAddr,
}

impl BootInfoFrameAllocator {
    /// Create a FrameAllocator from the passed memory map.
    ///
    /// # Safety
    /// The caller must guarantee that the passed memory map is valid.
    /// The main requirement is that all frames that are marked as `USABLE` in it are really unused
    pub unsafe fn init(regions: &'static MemoryRegions) -> Self {
        BootInfoFrameAllocator {
            regions,
            next: 0,
            last_phys_addr: PhysAddr::new(0),
        }
    }

    /// Returns an iterator over the usable frames specified in the memory map.
    fn usable_frames(&self) -> impl Iterator<Item = PhysFrame> + '_ {
        // get usable regions from memory map
        let regions = self.regions.iter();
        let usable_regions = regions.filter(|r| r.kind == MemoryRegionKind::Usable);
        // map each region to its address range
        let addr_ranges = usable_regions.map(|r| r.start..r.end);
        // transform to an iterator of frame start addresses
        let frame_addresses = addr_ranges.flat_map(|r| r.step_by(4096));
        // create `PhysFrame` types from the start addresses
        frame_addresses.map(|addr| PhysFrame::containing_address(PhysAddr::new(addr)))
    }
}

unsafe impl FrameAllocator<Size4KiB> for BootInfoFrameAllocator {
    fn allocate_frame(&mut self) -> Option<PhysFrame> {
        let frame = self.usable_frames().nth(self.next);
        frame.map(|f| {
            self.last_phys_addr = f.start_address();
            self.next += 1;
            f
        })
    }
}

impl FrameDeallocator<Size4KiB> for BootInfoFrameAllocator {
    unsafe fn deallocate_frame(&mut self, frame: PhysFrame<Size4KiB>) {
        if frame.start_address() == self.last_phys_addr {
            // safe to delete last frame
            self.next -= 1;
        }
    }
}
