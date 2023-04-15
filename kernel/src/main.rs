#![no_std]
#![no_main]
#![deny(unsafe_op_in_unsafe_fn)]
#![feature(naked_functions)]
#![feature(asm_const)]
#![feature(custom_test_frameworks)]
#![test_runner(zulu_os::test_runner)]
#![reexport_test_harness_main = "test_main"]

extern crate alloc;

use {
    bootloader_api::BootInfo,
    core::{arch::asm, num::NonZeroU64, panic::PanicInfo},
    x86_64::{
        registers::rflags::RFlags,
        structures::paging::{FrameAllocator, Mapper, Page, PageTableFlags},
        VirtAddr,
    },
    zulu_os::{memory, memory::BootInfoFrameAllocator, syscall},
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

#[no_mangle]
#[naked]
pub extern "C" fn _start(boot_info: &'static mut BootInfo) -> ! {
    unsafe {
        asm!(
            // Set second argument to the current rsp so we have access to it
            // `kernel_main` uses the C calling convention so it will see the correct args
            "mov rsi, rsp",
            "jmp kernel_main",
            options(noreturn)
        )
    }
}

#[no_mangle]
extern "C" fn kernel_main(boot_info: &'static mut BootInfo, rsp: u64) -> ! {
    let boot_info: &'static _ = boot_info;
    zulu_os::init(boot_info);

    let phys_mem_offset = VirtAddr::new(
        boot_info
            .physical_memory_offset
            .into_option()
            .expect("Physical memory offset required"),
    );
    // SAFETY:
    // 1. interrupts are disabled as they off by default, and havent been enabled yet
    // 2. The bootloader has mapped all of physical memory at `physical_memory_offset`
    let mut frame_allocator = unsafe { memory::init(phys_mem_offset) }.with(|mapper| {
        // setup heap while we have mapper
        let mut frame_allocator = unsafe { BootInfoFrameAllocator::init(&boot_info.memory_regions) };

        unsafe { zulu_os::allocator::init_kernel_heap(mapper, &mut frame_allocator) }
            .expect("Failed to init heap");
        frame_allocator
    });

    syscall::init_thread_data(syscall::ThreadData {
        kernel_rsp: NonZeroU64::new(rsp),
        user_tmp_rsp: None,
    });

    #[cfg(test)]
    test_main();

    let lowest_stack_page = Page::containing_address(VirtAddr::new(0xDEADBEEF));
    let stack_size = 4096u64 * 4;
    let highest_stack_page =
        Page::containing_address(lowest_stack_page.start_address() + stack_size);
    let user_stack = Page::range(lowest_stack_page, highest_stack_page);

    let flags =
        PageTableFlags::PRESENT | PageTableFlags::USER_ACCESSIBLE | PageTableFlags::WRITABLE;

    // SAFETY: 1. Interrupts are disabled 2. `memory::init` has been called 3. No recursion
    let bin = unsafe { memory::mapper() }.with(|mapper| {
        for page in user_stack {
            let frame = frame_allocator.allocate_frame().unwrap();
            unsafe {
                mapper
                    .map_to(page, frame, flags, &mut frame_allocator)
                    .unwrap()
                    .flush();
            };
        }

        zulu_os::elf::load(CHILD_PROCESS, mapper, &mut frame_allocator)
    });

    let top_of_stack = lowest_stack_page.start_address().as_u64() + stack_size;

    unsafe { enter_user_mode(bin.entry_point.as_u64(), top_of_stack) };
}

/// Sets the CPU to user mode (Ring 3) and jumps to `addr` using the stack starting at `user_stack`
/// Also enables interrupts
#[no_mangle]
#[naked]
pub unsafe extern "sysv64" fn enter_user_mode(addr: u64, user_stack: u64) -> ! {
    unsafe {
        asm!(
            // rip gets set to rcx when sysret is invoked, so write our first parameter there
            "mov rcx, rdi",
            "mov r11, {user_flags}",
            "mov rsp, rsi", // setup stack with `user_stack` (second param)
            "mov rbp, rsi",
            "swapgs",
            "sysretq",
            user_flags = const user_mode_flags(),
            options(noreturn)
        )
    };
}

const fn user_mode_flags() -> usize {
    // enable interrupts while in user mode
    RFlags::INTERRUPT_FLAG.bits() as usize
        // set the "resered, always 1" flag
        | 2
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
