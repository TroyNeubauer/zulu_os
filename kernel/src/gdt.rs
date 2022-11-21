use core::arch::asm;
use x86_64::structures::gdt::{Descriptor, GlobalDescriptorTable, SegmentSelector};
use x86_64::structures::tss::TaskStateSegment;
use x86_64::VirtAddr;

use crate::println;

pub const DOUBLE_FAULT_STACK_INDEX: u16 = 0;
pub const PAGE_FAULT_STACK_INDEX: u16 = 1;

lazy_static::lazy_static! {
    static ref TSS: TaskStateSegment = {
        let mut tss = TaskStateSegment::new();
        tss.interrupt_stack_table[DOUBLE_FAULT_STACK_INDEX as usize] = {
            const STACK_SIZE: usize = 1024 * 20;
            static mut STACK: [u8; STACK_SIZE] = [0; STACK_SIZE];

            let stack_start = VirtAddr::from_ptr(unsafe { &STACK });
            stack_start + STACK_SIZE
        };

        tss.interrupt_stack_table[PAGE_FAULT_STACK_INDEX as usize] = {
            const STACK_SIZE: usize = 1024 * 20;
            static mut STACK: [u8; STACK_SIZE] = [0; STACK_SIZE];

            let stack_start = VirtAddr::from_ptr(unsafe { &STACK });
            stack_start + STACK_SIZE
        };

        tss.privilege_stack_table[0] = {
            const STACK_SIZE: usize = 1024 * 20;
            static mut STACK: [u8; STACK_SIZE] = [0; STACK_SIZE];

            let stack_start = VirtAddr::from_ptr(unsafe { &STACK });
            stack_start + STACK_SIZE
        };

        tss
    };
}

lazy_static::lazy_static! {
    static ref GDT: (GlobalDescriptorTable, Selectors) = {
        let mut gdt = GlobalDescriptorTable::new();

        let kernel_code_selector = gdt.add_entry(Descriptor::kernel_code_segment());
        let kernel_data_selector = gdt.add_entry(Descriptor::kernel_data_segment());

        let user_data_selector = gdt.add_entry(Descriptor::user_data_segment());
        let user_code_selector = gdt.add_entry(Descriptor::user_code_segment());

        let tss_selector = gdt.add_entry(Descriptor::tss_segment(&TSS));
        let kernel_data_selector2 = gdt.add_entry(Descriptor::kernel_data_segment());

        let selectors = Selectors {
            kernel_code_selector,
            kernel_data_selector,
            user_code_selector,
            user_data_selector,
            tss_selector,
            kernel_data_selector2,
        };

        (gdt, selectors)
    };
}

struct Selectors {
    kernel_code_selector: SegmentSelector,
    kernel_data_selector: SegmentSelector,
    user_code_selector: SegmentSelector,
    user_data_selector: SegmentSelector,
    tss_selector: SegmentSelector,
    kernel_data_selector2: SegmentSelector,
}

#[no_mangle]
extern "sysv64" fn syscall_handler_inner(
    syscall_num: usize,
    arg0: usize,
    arg1: usize,
    arg2: usize,
    arg3: usize,
    arg4: usize,
) {
    println!(
        "SYSCALL: num: {}, 0: 0x{:X}, 1: 0x{:X} , 2: 0x{:X} , 3: 0x{:X} , 4: 0x{:X}",
        syscall_num, arg0, arg1, arg2, arg3, arg4
    );
}

#[naked]
#[no_mangle]
extern "x86-interrupt" fn syscall_handler() {
    // RBX, RSP, RBP, and R12–R15, need to be preserved inside syscall
    unsafe {
        asm!(
            // TODO: switch to kernel stack
            //
            // Registers on entry:
            //
            // rcx  return address
            // r11  saved rflags
            // rdi  system call number
            // rsi  arg0
            // rdx  arg1
            // r10  arg2
            // r8   arg3
            // r9   arg4
            //
            // Push return address
            "push rcx",
            // Push saved rflags
            "push r11",
            // SystemV expects registers in the folowing order for calling syscall_handler_inner
            //
            // rdi  syscall number
            // rsi  arg0
            // rdx  arg1
            // rcx  arg2 => gets set by syscall to return pointer
            // r8   arg3
            // r9   arg4
            "mov rcx, r10",

            "call syscall_handler_inner",

            "pop r11",
            "pop rcx",
            // TODO: restore user stack
            // "mov rsp, rsi",
            // "mov rbp, rsi",
            "sysretq",

            // TODO: Fix stack alignment so we can return properly
            options(noreturn)
        )
    };
}

pub fn init() {
    use x86_64::instructions::segmentation::{CS, DS, GS};
    use x86_64::instructions::tables::load_tss;
    use x86_64::registers::model_specific::{Efer, EferFlags, LStar, Star};
    use x86_64::registers::segmentation::Segment;

    GDT.0.load();
    let syscall_rip = VirtAddr::new(syscall_handler as usize as u64);
    unsafe {
        CS::set_reg(GDT.1.kernel_code_selector);
        DS::set_reg(GDT.1.kernel_data_selector);
        GS::set_reg(GDT.1.kernel_data_selector2);
        Star::write(
            GDT.1.user_code_selector,
            GDT.1.user_data_selector,
            GDT.1.kernel_code_selector,
            GDT.1.kernel_data_selector,
        )
        .unwrap();
        LStar::write(syscall_rip);
        Efer::update(|f| f.set(EferFlags::SYSTEM_CALL_EXTENSIONS, true));
        load_tss(GDT.1.tss_selector);
    }
}
/*
u cs: 27
u ds: 35
k cs: 8
k ds: 16
*/
