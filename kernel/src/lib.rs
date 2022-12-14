#![no_std]
#![cfg_attr(test, no_main)]
#![feature(custom_test_frameworks)]
#![feature(alloc_error_handler)]
#![feature(naked_functions)]
#![feature(asm_const)]
#![feature(abi_x86_interrupt)]
#![test_runner(crate::test_runner)]
#![reexport_test_harness_main = "test_main"]
#![deny(unsafe_op_in_unsafe_fn)]

//! # Zulu OS
//! 
//! A toy micro-kernel written in the Rust Programming Language to learn about operating systems and as the final project for my CS420 Operating Systems class.
//! 
//! ## Running
//! Zulu-OS can be run both inside Qemu and on a native machine via a bootable USB.
//! Booting in the emulator is better supported and significantly easier to setup, so the steps are described below.
//! 
//! 1. Install the qemu-x64 package for your Linux distributition.
//! 2. Install [Rust](https://www.rust-lang.org/tools/install)
//! 3. Install the [Bootimage](https://github.com/rust-osdev/bootimage) Rust binary package: `cargo install bootimage`
//! 4. `cd` into the kernel directory: `cd kernel`
//! 5. Run it! `cargo run`
//! 
//! NOTE: The first time may take a few minutes while `cargo` downloads all the dependencies, compiles the standard library from scratch plus Zulu-OS for our special CPU target
//! 
//! ## Design
//! 
//! 
//! ### Paging format
//! 
//! Zulu-OS maps all physical memory at a fixed fixed virtual address, making it trivially easy to modify any physical page and thus, modify or create any page mapping.
//! Fixed offset mapping simplifies the process of allocating new kernel memory as well as loading userspace processes.
//! 
//! 
//! ### Bootloading
//! 
//! Zulu-OS uses the excellent Rust [bootloader](https://github.com/rust-osdev/bootloader) crate to perform low-level initilization from 16-bit mode to 64-bit mode.
//! The bootloader crate also provides the kernel's entry point with boot info about which page ranges are already in use.
//! This allowed me to focus on the design of the kernel without having to worry about writing a correct bootleader.
//! 
//! 
//! ### Process loading and execution
//! Zulu-OS supports a very primitive process loading model that takes an elf file with no reinterpreter or relocations, loads it into memory, and jumps to the entry point.
//! Along with the limited syscall interface described below, dynamically loaded programs can read from the keyboard, write text to the screen, and invoke the exit syscall to stop themselves.
//! 
//! 
//! ### Syscalls
//! 
//! Zulu-OS currently supports three user space syscalls:
//! 1. Read. A userspace program can read one or more bytes from the keyboard.
//! 2. Write. A userspace program can ask the kernel to print the given text by writing to the VGA buffer.
//! 3. Exit. 
//!
//! This set of syscalls, while limited, it does allow for creation of simple games and text programs running in userspace. 
//! The current test userspace program that is run after the kernel is initialized (found inside the [userspace_test](./userspace_test/) directory)
//! calls write to show that printing works, and then calls exit.
//! A goal of this project is to extend the available syscalls to allow for more complex programs without compromising the security of the kernel.
//! 
//! A userspace syscall library is provided in [syscall](./syscall/) directory and used in [userspace_test](./userspace_test/).
//! 
//! 
//! ### Kernel Memory Allocation
//! 
//! We use a bump allocator that is given 100KB worth of pages on kernel init. Memory is never reclaimed.
//! We did this to keep the allocation implementation simple due to kernel memory rarely being allocated.
//! 
//! ### Interrupt handling
//! 
//! Illegal instructions, page faults, and floating point exceptions are currently not handled while executing in user mode, which causes a kernel panic.
//! More work is needed on the scheduler to make processes dynamic enough to support stopping at any time
//! 
//! 
//! ### Scheduler
//! 
//! Once the kernel is initialized the embedded userspace binary is executed.
//! Execution occurs until the userspace program either crashes the kernel or invokes the exit syscall.
//! Because only running a single process is currently supported, the kernel enters a wait-for-interrupt loop to save power until the CPU it is reset.
//! This will be expanded upon in the future to context switch to another process
//!
//! ### Testing
//!
//! Like any other complex project, testing is essential to ensuring functionality while preventing
//! regressions. Qemu is used to execute the integration tests inside [kernel/tests](./kernel/tests)
//! in the same context thet we run the OS in, as well as isolated from one another
//!

extern crate alloc;

use bootloader::BootInfo;
use core::panic::PanicInfo;

pub mod allocator;
pub mod elf;
pub mod gdt;
pub mod interrupts;
pub mod memory;
pub mod serial;
pub mod sys;
pub mod syscall;
pub mod task;
pub mod vga_buffer;

pub fn init(_boot_info: &'static BootInfo) {
    gdt::gdt_init();
    interrupts::init_idt();
    unsafe { interrupts::PICS.lock().initialize() };
    syscall::init();
}

pub trait Testable {
    fn run(&self);
}

impl<T> Testable for T
where
    T: Fn(),
{
    fn run(&self) {
        serial_print!("{}...\t", core::any::type_name::<T>());
        self();
        serial_println!("[ok]");
    }
}

pub fn test_runner(tests: &[&dyn Testable]) {
    serial_println!("Running {} tests", tests.len());
    for test in tests {
        test.run();
    }
    exit_qemu(QemuExitCode::Success);
}

pub fn test_panic_handler(info: &PanicInfo) -> ! {
    serial_println!("[failed]\n");
    serial_println!("Error: {}\n", info);
    exit_qemu(QemuExitCode::Failed);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum QemuExitCode {
    Success = 0x10,
    Failed = 0x11,
}

pub fn exit_qemu(exit_code: QemuExitCode) -> ! {
    use x86_64::instructions::port::Port;

    unsafe {
        let mut port = Port::new(0xf4);
        port.write(exit_code as u32);
    }
    crate::sys::hlt_loop();
}

#[cfg(test)]
bootloader::entry_point!(kernel_main_test);

#[cfg(test)]
fn kernel_main_test(boot_info: &'static BootInfo) -> ! {
    init(boot_info);
    test_main();
    crate::sys::hlt_loop();
}

#[cfg(test)]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    test_panic_handler(info)
}

#[alloc_error_handler]
fn alloc_error_handler(layout: alloc::alloc::Layout) -> ! {
    panic!("allocation error: {:?}", layout)
}
