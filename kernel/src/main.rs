#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(zulu_os::test_runner)]
#![reexport_test_harness_main = "test_main"]

use alloc::collections::BTreeMap;

extern crate alloc;

use {
    alloc::vec::Vec,
    bootloader::BootInfo,
    core::{cmp, mem, ops::Range, panic::PanicInfo, slice, str::FromStr},
    elf::{endian::LittleEndian, section::SectionHeader, ElfBytes},
    x86_64::structures::paging::{page::PageRangeInclusive, PageTableFlags},
    x86_64::{
        structures::paging::{FrameAllocator, Mapper, Page},
        VirtAddr,
    },
    zulu_os::{memory::BootInfoFrameAllocator, println, task::executor::Executor, task::Task},
};

bootloader::entry_point!(kernel_main);

const CHILD_PROCESS: &'static [u8] = include_bytes!("../processes/userspace_test").as_slice();

#[no_mangle]
pub extern "C" fn my_cool_function(x: usize) -> usize {
    x
}

fn kernel_main(boot_info: &'static BootInfo) -> ! {
    let mut x: usize = 0;
    let func: extern "C" fn(usize) -> usize = my_cool_function;
    unsafe { core::ptr::write_volatile(&mut x, func as usize) };
    let _ = unsafe { core::ptr::read_volatile(&mut x) };

    zulu_os::init(boot_info);

    let phys_mem_offset = VirtAddr::new(boot_info.physical_memory_offset);
    let mut mapper = unsafe { zulu_os::memory::init(phys_mem_offset) };

    let mut frame_allocator = unsafe { BootInfoFrameAllocator::init(&boot_info.memory_map) };

    unsafe { zulu_os::allocator::init_kernel_heap(&mut mapper, &mut frame_allocator) }
        .expect("Failed to init heap");

    #[cfg(test)]
    test_main();

    let elf = ElfBytes::<LittleEndian>::minimal_parse(CHILD_PROCESS).expect("Open test1");
    let (elf_sections, strtab) = elf.section_headers_with_strtab().unwrap();
    let (elf_sections, strtab) = (elf_sections.unwrap(), strtab.unwrap());
    println!("entry: 0x{:X?}", elf.ehdr.e_entry);
    println!("type: {:?}", elf.ehdr.e_type);

    #[derive(Copy, Clone, PartialEq, Eq, Debug)]
    enum ElfSectionName {
        Text,
        Rodata,
        Comment,
        Bss,
    }

    impl FromStr for ElfSectionName {
        type Err = ();

        fn from_str(s: &str) -> Result<Self, Self::Err> {
            Ok(match s {
                ".text" => ElfSectionName::Text,
                ".rodata" => ElfSectionName::Rodata,
                ".comment" => ElfSectionName::Comment,
                ".bss" => ElfSectionName::Bss,
                _ => return Err(()),
            })
        }
    }

    #[derive(Copy, Clone, PartialEq, Eq, Debug)]
    enum ElfSectionType {
        Ignore,
        ProgramBits,
        SymbolTable,
        StringTable,
        Relocations,
        Hash,
        Dynamic,
        Note,
        NoBits,
        Rel,
        Shlib,
        Dynsym,
        InitArray,
        FiniArray,
        PreInitArray,
        Group,
        SymtabShndx,
        Loos,
        GnuAttributes,
        GnuHash,
    }

    impl TryFrom<u32> for ElfSectionType {
        type Error = ();

        fn try_from(value: u32) -> Result<Self, Self::Error> {
            use object::elf::*;
            Ok(match value {
                SHT_NULL => ElfSectionType::Ignore,
                SHT_PROGBITS => ElfSectionType::ProgramBits,
                SHT_SYMTAB => ElfSectionType::SymbolTable,
                SHT_STRTAB => ElfSectionType::StringTable,
                SHT_RELA => ElfSectionType::Relocations,
                SHT_HASH => ElfSectionType::Hash,
                SHT_DYNAMIC => ElfSectionType::Dynamic,
                SHT_NOTE => ElfSectionType::Note,
                SHT_NOBITS => ElfSectionType::NoBits,
                SHT_REL => ElfSectionType::Rel,
                SHT_SHLIB => ElfSectionType::Shlib,
                SHT_DYNSYM => ElfSectionType::Dynsym,
                SHT_INIT_ARRAY => ElfSectionType::InitArray,
                SHT_FINI_ARRAY => ElfSectionType::FiniArray,
                SHT_PREINIT_ARRAY => ElfSectionType::PreInitArray,
                SHT_GROUP => ElfSectionType::Group,
                SHT_SYMTAB_SHNDX => ElfSectionType::SymtabShndx,
                SHT_LOOS => ElfSectionType::Loos,
                SHT_GNU_ATTRIBUTES => ElfSectionType::GnuAttributes,
                SHT_GNU_HASH => ElfSectionType::GnuHash,
                _ => return Err(()),
            })
        }
    }

    #[derive(Debug, Copy, Clone)]
    struct VirtualMapDestination {
        /// Virtual address where this section section will be mapped into memory
        start: VirtAddr,
        /// The pages occupied by this section, this may overlap with pages in other sections if
        /// sections are not page aligned
        pages: PageRangeInclusive,
        /// The original address the elf file wanted
        /// Same as `addr` but can be offset
        original_addr: u64,
    }

    #[derive(Debug, Clone)]
    struct MappedElfSection {
        name: ElfSectionName,
        ty: ElfSectionType,
        flags: PageTableFlags,
        /// Offset into the elf file where this section data resides
        file_range: Range<usize>,
        addr: VirtualMapDestination,
    }

    impl MappedElfSection {
        fn len(&self) -> usize {
            self.file_range.len()
        }
    }

    struct ElfFile {
        sections: Vec<MappedElfSection>,
        entry_point: VirtAddr,
    }

    let map_vaddr = |src: u64| -> VirtAddr {
        if src == 0 {
            panic!("cannot map null address to new address!");
        }
        let dst = VirtAddr::new(src) + 0x460u64 * 4096;
        println!("  {:?} -> {:?}", src, dst);
        dst
    };

    let mut sections = Vec::new();
    for section in elf_sections {
        let name = strtab.get(section.sh_name as usize).unwrap();
        let Ok(parsed_name) = name.parse() else {
            println!("Ignoring unknown section name: {}", name);
            continue;
        };
        let Ok(parsed_ty) = section.sh_type.try_into() else {
            println!("Ignoring unknown section type: {} for {}", section.sh_type, name);
            continue;
        };
        if parsed_ty == ElfSectionType::Ignore {
            continue;
        }
        use object::elf::*;
        fn parse_section_flags(section: &SectionHeader) -> PageTableFlags {
            let read = true;
            let write = section.sh_flags & SHF_WRITE as u64 != 0;
            let execute = section.sh_flags & SHF_EXECINSTR as u64 != 0;

            let mut flags = PageTableFlags::PRESENT;
            if read {
                flags |= PageTableFlags::USER_ACCESSIBLE;
            }
            if write {
                flags |= PageTableFlags::WRITABLE;
            }
            if !execute {
                flags |= PageTableFlags::NO_EXECUTE;
            }
            flags
        }

        if section.sh_addr == 0 {
            continue;
        }

        let addr = {
            let original_addr = section.sh_addr;
            let virtual_start = map_vaddr(original_addr);

            let virtual_page_start = Page::containing_address(virtual_start);
            let virtual_page_end = Page::containing_address(virtual_start + section.sh_size);
            let pages = Page::range_inclusive(virtual_page_start, virtual_page_end);

            VirtualMapDestination {
                start: virtual_start,
                pages,
                original_addr,
            }
        };

        let file_range =
            (section.sh_offset as usize)..(section.sh_offset as usize + section.sh_size as usize);
        sections.push(MappedElfSection {
            name: parsed_name,
            ty: parsed_ty,
            flags: parse_section_flags(&section),
            file_range,
            addr,
        })
    }
    let elf_file = ElfFile {
        sections,
        entry_point: map_vaddr(elf.ehdr.e_entry),
    };
    let mut pages_to_map = BTreeMap::<Page, PageTableFlags>::new();

    for section in &elf_file.sections {
        for page in section.addr.pages {
            pages_to_map
                .entry(page)
                .and_modify(|current| {
                    let new = section.flags;
                    *current |= new;
                    let nx = PageTableFlags::NO_EXECUTE;
                    if !current.contains(nx) || !new.contains(nx) {
                        // if either one is executable, clear execute bit
                        current.set(nx, false);
                        println!("clearing execute bit for page: {:?}", page);
                    }
                })
                .or_insert(section.flags);
        }
    }
    println!("mapping: {:#?}", pages_to_map);
    for (&page, &original_flags) in &pages_to_map {
        let frame = frame_allocator.allocate_frame().unwrap();
        // unconditionally add writable flag so we can copy the section data to it
        let flags = original_flags | PageTableFlags::WRITABLE;

        println!("  mapping {:?} to {:?} with {:?}", page, frame, flags);
        unsafe {
            mapper
                .map_to(page, frame, flags, &mut frame_allocator)
                .unwrap()
                .flush();
        };
    }

    for section in &elf_file.sections {
        let section_src = &CHILD_PROCESS[section.file_range.clone()];
        let section_dst: &mut [u8] =
            unsafe { slice::from_raw_parts_mut(section.addr.start.as_mut_ptr(), section.len()) };
        section_dst.copy_from_slice(section_src);
        let to_print = &section_src[..cmp::min(16, section_src.len())];
        println!("  {:?}-{:?}: {:X?}", section.name, section.ty, to_print);
    }

    // remove writable flag for pages that dont want to be writable
    for (&page, &original_flags) in &pages_to_map {
        if !original_flags.contains(PageTableFlags::WRITABLE) {
            unsafe {
                mapper.update_flags(page, original_flags).unwrap().flush();
            }
        }
    }

    for section in &elf_file.sections {
        let section_bytes: &[u8] =
            unsafe { slice::from_raw_parts(section.addr.start.as_ptr(), section.len()) };
        let to_print = &section_bytes[..cmp::min(16, section.len())];
        println!(
            " section {:?}: {:X?} from {:X}",
            section.name, to_print, section.addr.original_addr
        );
    }

    println!("Base address");
    let entry_point = elf_file.entry_point.as_ptr();
    let val = unsafe { *entry_point };
    println!("Jumping to addr: {:?}: {}", entry_point, val);
    println!("");
    unsafe { jmp(entry_point) }
    println!("");

    let mut executor = Executor::new();
    executor.spawn(Task::new(zulu_os::task::keyboard::print_keypresses()));
    executor.run();
}

#[no_mangle]
#[inline(never)]
pub unsafe extern "C" fn jmp(addr: *const u8) {
    let fn_ptr: fn() = mem::transmute(addr);
    fn_ptr();
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
