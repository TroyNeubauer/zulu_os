#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(zulu_os::test_runner)]
#![reexport_test_harness_main = "test_main"]

extern crate alloc;

use {
    bootloader::BootInfo,
    core::panic::PanicInfo,
    x86_64::VirtAddr,
    zulu_os::{memory::BootInfoFrameAllocator, task::executor::Executor, task::Task},
};

#[repr(C)] // guarantee 'bytes' comes after '_align'
pub struct AlignedAs<Align, Bytes: ?Sized> {
    pub _align: [Align; 0],
    pub bytes: Bytes,
}

macro_rules! include_bytes_align_as {
    ($align_ty:ty, $path:literal) => {{
        // const block expression to encapsulate the static
        use $crate::AlignedAs;

        // this assignment is made possible by CoerceUnsized
        static ALIGNED: &AlignedAs<$align_ty, [u8]> = &AlignedAs {
            _align: [],
            bytes: *include_bytes!($path),
        };

        &ALIGNED.bytes
    }};
}

bootloader::entry_point!(kernel_main);

#[repr(align(4096))]
struct Align4096;

static CHILD_PROCESS: &[u8] = include_bytes_align_as!(Align4096, "../processes/userspace_test");

fn kernel_main(boot_info: &'static BootInfo) -> ! {
    zulu_os::init(boot_info);

    let phys_mem_offset = VirtAddr::new(boot_info.physical_memory_offset);
    let mut mapper = unsafe { zulu_os::memory::init(phys_mem_offset) };

    let mut frame_allocator = unsafe { BootInfoFrameAllocator::init(&boot_info.memory_map) };

    unsafe { zulu_os::allocator::init_kernel_heap(&mut mapper, &mut frame_allocator) }
        .expect("Failed to init heap");

    let _bin = zulu_os::elf::load(CHILD_PROCESS, &mut mapper, &mut frame_allocator);

    #[cfg(test)]
    test_main();

    let mut executor = Executor::new();
    executor.spawn(Task::new(zulu_os::task::keyboard::print_keypresses()));
    executor.run();
}

/// This function is called on panic.
#[cfg(not(test))]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    zulu_os::println!("{}", info);
    zulu_os::sys::hlt_loop();
}

#[cfg(test)]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    zulu_os::test_panic_handler(info)
}
