#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![feature(naked_functions)]
#![test_runner(zulu_os::test_runner)]
#![reexport_test_harness_main = "test_main"]

use x86_64::structures::paging::{
    page::{PageRange, PageRangeInclusive},
    FrameAllocator, Mapper, Page, PageTableFlags,
};
use zulu_os::println;

extern crate alloc;

use {
    bootloader::BootInfo,
    core::arch::asm,
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

#[repr(align(4096))]
struct Align4096;

static CHILD_PROCESS: &[u8] = include_bytes_align_as!(Align4096, "../processes/userspace_test");

bootloader::entry_point!(kernel_main);

fn kernel_main(boot_info: &'static BootInfo) -> ! {
    zulu_os::init(boot_info);

    let phys_mem_offset = VirtAddr::new(boot_info.physical_memory_offset);
    let mut mapper = unsafe { zulu_os::memory::init(phys_mem_offset) };

    let mut frame_allocator = unsafe { BootInfoFrameAllocator::init(&boot_info.memory_map) };

    unsafe { zulu_os::allocator::init_kernel_heap(&mut mapper, &mut frame_allocator) }
        .expect("Failed to init heap");

    #[cfg(test)]
    test_main();

    let lowest_stack_page = Page::containing_address(VirtAddr::new(0xDEADBEEF));
    let stack_size = 4096u64 * 4;
    let highest_stack_page =
        Page::containing_address(lowest_stack_page.start_address() + stack_size);
    let user_stack = Page::range(lowest_stack_page, highest_stack_page);

    let flags =
        PageTableFlags::PRESENT | PageTableFlags::USER_ACCESSIBLE | PageTableFlags::WRITABLE;

    for page in user_stack {
        let frame = frame_allocator.allocate_frame().unwrap();
        unsafe {
            mapper
                .map_to(page, frame, flags, &mut frame_allocator)
                .unwrap()
                .flush();
        };
    }

    let bin = zulu_os::elf::load(CHILD_PROCESS, &mut mapper, &mut frame_allocator);

    let top_of_stack = lowest_stack_page.start_address().as_u64() + stack_size;

    unsafe { enter_user_mode(bin.entry_point.as_u64(), top_of_stack) };

    /*
    bin.run();

    let mut executor = Executor::new();
    executor.spawn(Task::new(zulu_os::task::keyboard::print_keypresses()));
    executor.run();
    */
}

#[no_mangle]
#[naked]
pub unsafe extern "sysv64" fn enter_user_mode(addr: u64, user_stack: u64) -> ! {
    unsafe {
        asm!(
            // rip gets set to rcx when sysret is invoked, so write our first parameter there
            "mov rcx, rdi",
            "mov r11, 0x202",
            "mov rsp, rsi", // setup stack with `user_stack` (second param)
            "mov rbp, rsi",
            "sysretq",
            options(noreturn)
        )
    };
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
