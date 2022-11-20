use crate::println;
use pic8259::ChainedPics;

use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame, PageFaultErrorCode};

pub const PIC_1_OFFSET: u8 = 32;
pub const PIC_2_OFFSET: u8 = PIC_1_OFFSET + 8;

lazy_static::lazy_static! {
    static ref IDT: InterruptDescriptorTable = {
        let mut idt = InterruptDescriptorTable::new();
        idt.breakpoint.set_handler_fn(breakpoint_handler);
        let double_fault_ops = idt.double_fault.set_handler_fn(double_fault_handler);
        unsafe {
            double_fault_ops.set_stack_index(crate::gdt::DOUBLE_FAULT_STACK_INDEX)
        };
        let page_fault_ops = idt.page_fault.set_handler_fn(page_fault_handler);
        unsafe {
            page_fault_ops.set_stack_index(crate::gdt::PAGE_FAULT_STACK_INDEX)
        };
        idt[InterruptIndex::Timer.as_usize()].set_handler_fn(timer_interrupt_handler);
        idt[InterruptIndex::Keyboard.as_usize()].set_handler_fn(keyboard_interrupt_handler);
        idt
    };
}

pub static PICS: spin::Mutex<ChainedPics> =
    spin::Mutex::new(unsafe { ChainedPics::new(PIC_1_OFFSET, PIC_2_OFFSET) });

pub fn init_idt() {
    IDT.load();
}

extern "x86-interrupt" fn breakpoint_handler(frame: InterruptStackFrame) {
    println!("Breakpoint hit: {:#?}", frame);
}

extern "x86-interrupt" fn double_fault_handler(frame: InterruptStackFrame, code: u64) -> ! {
    panic!("DOUBLE FAULT. Code: {}\n{:#?}", code, frame)
}

extern "x86-interrupt" fn page_fault_handler(frame: InterruptStackFrame, code: PageFaultErrorCode) {
    panic!("PAGE FAULT. Code: {:?}\n{:#?}", code, frame)
}

extern "x86-interrupt" fn timer_interrupt_handler(_frame: InterruptStackFrame) {
    unsafe {
        PICS.lock()
            .notify_end_of_interrupt(InterruptIndex::Timer.as_u8())
    };
}

extern "x86-interrupt" fn keyboard_interrupt_handler(_frame: InterruptStackFrame) {
    use x86_64::instructions::port::Port;

    let mut port = Port::new(0x60);
    let scancode: u8 = unsafe { port.read() };
    crate::task::keyboard::add_scancode(scancode);

    unsafe {
        PICS.lock()
            .notify_end_of_interrupt(InterruptIndex::Timer.as_u8())
    };
}

#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum InterruptIndex {
    Timer = PIC_1_OFFSET,
    Keyboard,
}

impl InterruptIndex {
    fn as_u8(self) -> u8 {
        self as u8
    }

    fn as_usize(self) -> usize {
        usize::from(self.as_u8())
    }
}
