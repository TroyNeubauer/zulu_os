#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]

use zulu_os::serial_print;

#[no_mangle]
pub extern "C" fn _start() -> ! {
    serial_print!("stack_overflow::stack_overflow...\t");

    zulu_os::gdt::gdt_init();
    TEST_IDT.load();

    x86_64::instructions::interrupts::int3();
    panic!("Execution continued after breakpoint");
}

lazy_static::lazy_static! {
    static ref TEST_IDT: InterruptDescriptorTable = {
        let mut idt = InterruptDescriptorTable::new();
        idt.breakpoint.set_handler_fn(test_breakpoint_handler);
        idt
    };
}

use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame};
use zulu_os::{exit_qemu, QemuExitCode};

extern "x86-interrupt" fn test_breakpoint_handler(_stack_frame: InterruptStackFrame) {
    exit_qemu(QemuExitCode::Success);
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    zulu_os::test_panic_handler(info)
}
