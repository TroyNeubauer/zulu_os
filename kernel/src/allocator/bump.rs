use core::sync::atomic::{AtomicUsize, Ordering};

pub struct BumpAllocator {
    heap_start: AtomicUsize,
    heap_end: AtomicUsize,
    next: AtomicUsize,
    //allocations: AtomicUsize,
}

impl BumpAllocator {
    /// Creates a new empty bump allocator.
    pub const fn new() -> Self {
        BumpAllocator {
            heap_start: AtomicUsize::new(0),
            heap_end: AtomicUsize::new(0),
            next: AtomicUsize::new(0),
            //allocations: AtomicUsize::new(0),
        }
    }

    /// Initializes the bump allocator with the given heap bounds.
    ///
    /// # Safety
    /// 1. The caller must ensure that the given memory range is unused.
    /// 2. This method must be called only once.
    pub unsafe fn init(&self, heap_start: usize, heap_size: usize) {
        self.heap_start.store(heap_start, Ordering::SeqCst);
        self.heap_end
            .store(heap_start + heap_size, Ordering::SeqCst);
        self.next.store(heap_start, Ordering::SeqCst);
    }
}

use alloc::alloc::{GlobalAlloc, Layout};

unsafe impl GlobalAlloc for BumpAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // # Safety: we assume layout.align is a power of two
        // FIXME is this true?
        let size = unsafe { super::align_up(layout.size(), layout.align()) };
        let new_ptr = self.next.fetch_add(size, Ordering::AcqRel);
        if new_ptr > self.heap_end.load(Ordering::Relaxed) {
            core::ptr::null_mut()
        } else {
            new_ptr as *mut u8
        }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // # Safety: Same as above
        // FIXME
        let size = unsafe { super::align_up(layout.size(), layout.align()) };
        let current_ptr = self.next.load(Ordering::Acquire);
        if ptr as usize + size == current_ptr {
            // We are freeing the pointer we most recently allocated, so we can move `self.next`
            // back if no other threads beat us

            // Best effort subtraction:
            // If another thread races between the load and this store, too bad
            let _ = self.next.compare_exchange(
                current_ptr,
                current_ptr - size,
                Ordering::AcqRel,
                Ordering::Relaxed,
            );
        }
    }
}
