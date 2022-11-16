#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]

use zulu_os::serial_print;

#[no_mangle]
pub extern "C" fn _start() -> ! {
    serial_print!("stack_overflow::stack_overflow...\t");

    zulu_os::gdt::init();
    init_test_idt();

    // trigger a stack overflow
    stack_overflow();

    panic!("Execution continued after stack overflow");
}

#[allow(unconditional_recursion)]
fn stack_overflow() {
    stack_overflow(); // for each recursion, the return address is pushed
    let _ = volatile::Volatile::new(0).read(); // prevent tail recursion optimizations
}

use x86_64::structures::idt::InterruptDescriptorTable;

lazy_static::lazy_static! {
    static ref TEST_IDT: InterruptDescriptorTable = {
        let mut idt = InterruptDescriptorTable::new();
        let double_fault_ops = idt.double_fault.set_handler_fn(test_double_fault_handler);
        unsafe {
            double_fault_ops.set_stack_index(zulu_os::gdt::DOUBLE_FAULT_STACK_INDEX);
        }

        idt
    };
}

pub fn init_test_idt() {
    TEST_IDT.load();
}

use x86_64::structures::idt::InterruptStackFrame;
use zulu_os::{exit_qemu, serial_println, QemuExitCode};

extern "x86-interrupt" fn test_double_fault_handler(
    _stack_frame: InterruptStackFrame,
    _error_code: u64,
) -> ! {
    serial_println!("[ok]");
    exit_qemu(QemuExitCode::Success);
    zulu_os::sys::hlt_loop();
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    zulu_os::test_panic_handler(info)
}
