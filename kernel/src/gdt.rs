use x86_64::instructions::segmentation::{CS, DS, GS};
use x86_64::structures::gdt::{Descriptor, GlobalDescriptorTable, SegmentSelector};
use x86_64::structures::tss::TaskStateSegment;
use x86_64::VirtAddr;

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
pub fn gdt_init() {
    use x86_64::instructions::tables::load_tss;
    use x86_64::registers::model_specific::Star;
    use x86_64::registers::segmentation::Segment;

    use raw_cpuid::CpuId;
    let cpuid = CpuId::new();

    let has_fsgbase = cpuid
        .get_extended_feature_info()
        .map_or(false, |info| info.has_fsgsbase());
    assert!(has_fsgbase);

    GDT.0.load();
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

        load_tss(GDT.1.tss_selector);
    }
}
