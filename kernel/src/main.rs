#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(zulu_os::test_runner)]
#![reexport_test_harness_main = "test_main"]

extern crate alloc;

use bootloader::BootInfo;
use core::panic::PanicInfo;
use zulu_os::task::executor::Executor;
use zulu_os::task::Task;

use x86_64::VirtAddr;

use zulu_os::memory::BootInfoFrameAllocator;

bootloader::entry_point!(kernel_main);

const CHILD_PROCESS: &'static [u8] = include_bytes!("../processes/userspace_test").as_slice();

fn kernel_main(boot_info: &'static BootInfo) -> ! {
    zulu_os::init(boot_info);

    let phys_mem_offset = VirtAddr::new(boot_info.physical_memory_offset);
    let mut mapper = unsafe { zulu_os::memory::init(phys_mem_offset) };

    let mut frame_allocator = unsafe { BootInfoFrameAllocator::init(&boot_info.memory_map) };

    unsafe { zulu_os::allocator::init_kernel_heap(&mut mapper, &mut frame_allocator) }
        .expect("Failed to init heap");

    #[cfg(test)]
    test_main();

    /*
    let mut executor = Executor::new();
    executor.spawn(Task::new(zulu_os::task::keyboard::print_keypresses()));
    executor.run();
    */
    let elf = ElfBytes::<AnyEndian>::minimal_parse(CHILD_PROCESS).expect("Open test1");
    loop {
    }
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
