#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(zulu_os::test_runner)]
#![reexport_test_harness_main = "test_main"]

extern crate alloc;

use {
    bootloader::BootInfo, core::panic::PanicInfo, elf::endian::LittleEndian, elf::ElfBytes,
    zulu_os::println, zulu_os::task::executor::Executor, zulu_os::task::Task,
};

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

    //#[cfg(test)]
    //test_main();

    let elf = ElfBytes::<LittleEndian>::minimal_parse(CHILD_PROCESS).expect("Open test1");
    let (sections, strtab) = elf.section_headers_with_strtab().unwrap();
    let (sections, strtab) = (sections.unwrap(), strtab.unwrap());
    println!("entry: 0x{:X?}", elf.ehdr.e_entry);
    println!("type: {:?}", elf.ehdr.e_type);
    for section in sections {
        use object::elf::*;
        let ty = match section.sh_type.try_into().unwrap() {
            ET_NONE => "none",
            ET_REL => "rel",
            ET_EXEC => "exec",
            ET_DYN => "dyn",
            ET_CORE => "core",
            _ => "unknown",
        };
        let read = true;
        let write = section.sh_flags & SHF_WRITE as u64 != 0;
        let execute = section.sh_flags & SHF_EXECINSTR as u64 != 0;
        let name = strtab.get(section.sh_name as usize).unwrap();
        let r = if read { "R" } else { "." };
        let w = if write { "W" } else { "." };
        let x = if execute { "X" } else { "." };

        let file_start = section.sh_offset as usize;
        let file_end = section.sh_offset + section.sh_size;
        let virtual_start = section.sh_addr;
        let virtual_end = section.sh_addr + section.sh_size;

        if name == ".text" {
            println!(
                "{:X} {:X}",
                CHILD_PROCESS[file_start],
                CHILD_PROCESS[file_start + 1]
            );
        }
        println!(
            "  {:16}: {}{}{}: {:X}..{:X} -> {:X} {:X} align: {}",
            name, r, w, x, file_start, file_end, virtual_start, virtual_end, section.sh_addralign
        );
    }

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
