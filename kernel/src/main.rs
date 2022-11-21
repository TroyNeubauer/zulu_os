#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(zulu_os::test_runner)]
#![reexport_test_harness_main = "test_main"]

use alloc::collections::BTreeMap;
use object::{elf::FileHeader64, Endianness, LittleEndian, read::elf::ProgramHeader};

extern crate alloc;

use num_enum::TryFromPrimitive;
use core::convert::TryFrom;

use {
    alloc::vec::Vec,
    bootloader::BootInfo,
    core::{cmp, mem, ops::Range, panic::PanicInfo, slice, str::FromStr},
    //elf::{endian::LittleEndian, section::SectionHeader, ElfBytes},
    x86_64::structures::paging::{page::PageRangeInclusive, PageTableFlags},
    x86_64::{
        structures::paging::{FrameAllocator, Mapper, Page},
        VirtAddr,
    },
    zulu_os::{memory::BootInfoFrameAllocator, println, task::executor::Executor, task::Task},
};

use object::read::elf::{FileHeader, Rel, Rela, SectionHeader};

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

bootloader::entry_point!(kernel_main);

#[repr(align(4096))]
struct Align4096;

static CHILD_PROCESS: &'static [u8] =
    include_bytes_align_as!(Align4096, "../processes/userspace_test");

const LOAD_TEXT_SECTION_AT: u64 = 0x660000;

fn kernel_main(boot_info: &'static BootInfo) -> ! {
    zulu_os::init(boot_info);

    let phys_mem_offset = VirtAddr::new(boot_info.physical_memory_offset);
    let mut mapper = unsafe { zulu_os::memory::init(phys_mem_offset) };

    let mut frame_allocator = unsafe { BootInfoFrameAllocator::init(&boot_info.memory_map) };

    unsafe { zulu_os::allocator::init_kernel_heap(&mut mapper, &mut frame_allocator) }
        .expect("Failed to init heap");

    #[cfg(test)]
    test_main();

    let elf = FileHeader64::<LittleEndian>::parse(CHILD_PROCESS).unwrap();
    let program_headers = elf.program_headers(LittleEndian, CHILD_PROCESS).unwrap();
    

    println!("entry: 0x{:X?}", elf.e_entry);
    println!("type: {:?}", elf.e_type);

    use object::elf::*;
    #[derive(Debug, Copy, Clone, Eq, PartialEq, TryFromPrimitive)]
    #[repr(u32)]
    enum ElfSegmentType {
        Null = PT_NULL,
        Load = PT_LOAD,
        Dynamic = PT_DYNAMIC,
        Interp = PT_INTERP,
        Note = PT_NOTE,
        Shlib = PT_SHLIB,
        Phdr = PT_PHDR,
        Tls = PT_TLS,
        Loos = PT_LOOS,
        GnuEhFrame = PT_GNU_EH_FRAME,
        GnuStack = PT_GNU_STACK,
        GnuRelo = PT_GNU_RELRO,
        Hios = PT_HIOS,
        Loproc = PT_LOPROC,
        Hiproc = PT_HIPROC,
    } 

    #[derive(Debug, Copy, Clone)]
    struct VirtualMapDestination {
        /// Virtual address where this segment will be mapped into memory.
        /// Starts with the same value as `original_start`, but can be changed.
        start: VirtAddr,
        /// The length of this virtual addr segment in bytes
        len: usize,
        /// The original virtual address the segment file wanted
        original_start: u64,
    }

    #[derive(Debug, Clone)]
    struct ElfSegment {
        ty: ElfSegmentType,
        align: usize,
        flags: PageTableFlags,
        /// Offset into the elf file where this segment data resides
        file_range: Range<usize>,
        addr: VirtualMapDestination,
    }

    struct ElfFile {
        segments: Vec<ElfSegment>,
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

    let mut segments = Vec::new();
    for segment in program_headers {
        let Ok(parsed_ty) = segment.p_type(LittleEndian).try_into() else {
            println!("Ignoring unknown segment type: {:?}", segment.p_type);
            continue;
        };
        if parsed_ty == ElfSegmentType::Null {
            continue;
        }
        use object::elf::*;
        fn parse_segment_flags(section: &ProgramHeader64<LittleEndian>) -> PageTableFlags {
            let bit_flags: u32 = section.p_flags(LittleEndian);

            let mut flags = PageTableFlags::PRESENT;
            if (bit_flags & PF_R) != 0 {
                flags |= PageTableFlags::USER_ACCESSIBLE;
            }
            if (bit_flags & PF_W) != 0 {
                flags |= PageTableFlags::WRITABLE;
            }
            if (bit_flags & PF_X) == 0 {
                flags |= PageTableFlags::NO_EXECUTE;
            }
            flags
        }

        if segment.p_memsz.get(LittleEndian) == 0 {
            continue;
        }

        let addr = {
            let original_start = segment.p_vaddr(LittleEndian);

            VirtualMapDestination {
                start: VirtAddr::new(original_start),
                len: segment.p_memsz(LittleEndian) as usize,
                original_start,
            }
        };

        let offset = segment.p_offset(LittleEndian) as usize;
        let file_size = segment.p_filesz(LittleEndian) as usize;
        segments.push(ElfSegment {
            ty: parsed_ty,
            flags: parse_segment_flags(&segment),
            file_range: offset..offset + file_size,
            addr,
            align: segment.p_align(LittleEndian) as usize,
        })
    }
    let mut elf_file = ElfFile {
        segments,
        entry_point: VirtAddr::new(elf.e_entry(LittleEndian)),
    };

    impl ElfFile {
        /// Remaps the elf file using the given virtual mapping function
        pub fn remap(&mut self, f: impl Fn(VirtAddr) -> VirtAddr) {
            self.entry_point = f(self.entry_point);
            for segment in &mut self.segments {
                segment.addr.remap(&f);
            }
        }
    }

    impl VirtualMapDestination {
        pub fn pages(&self) -> PageRangeInclusive {
            let virtual_page_start = Page::containing_address(self.start);
            let virtual_page_end = Page::containing_address(self.start + self.len);
            Page::range_inclusive(virtual_page_start, virtual_page_end)
        }

        /// Remaps the elf file using the given virtual mapping function
        pub fn remap(&mut self, f: &impl Fn(VirtAddr) -> VirtAddr) {
            self.start = f(self.start);
        }
    }

    let mut default_text_addr = None;
    for section in &elf_file.segments {
        if !section.flags.contains(PageTableFlags::NO_EXECUTE) {
            default_text_addr = Some(section.addr.start.as_u64());
        }
    }
    let offset_to_apply = LOAD_TEXT_SECTION_AT as i64 - default_text_addr.unwrap() as i64;

    println!("rel offset is 0x{:X?}", offset_to_apply);
    elf_file.remap(|src| VirtAddr::new((src.as_u64() as i64 + offset_to_apply) as u64));

    let mut pages_to_map = BTreeMap::<Page, PageTableFlags>::new();

    for segment in &elf_file.segments {
        for page in segment.addr.pages() {
            pages_to_map
                .entry(page)
                .and_modify(|current| {
                    let new = segment.flags;
                    *current |= new;
                    let nx = PageTableFlags::NO_EXECUTE;
                    if !current.contains(nx) || !new.contains(nx) {
                        // if either one is executable, clear execute bit
                        current.set(nx, false);
                        //println!("clearing execute bit for page: {:?}", page);
                    }
                })
                .or_insert(segment.flags);
        }
    }
    println!("mapping: {:?}", pages_to_map);
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

    for section in &elf_file.segments {
        
        if section.ty == ElfSegmentType::Load {
            let section_src = &CHILD_PROCESS[section.file_range.clone()];
            let section_dst: &mut [u8] =
                unsafe { slice::from_raw_parts_mut(section.addr.start.as_mut_ptr(), section.file_range.len()) };
            section_dst.copy_from_slice(section_src);
            let to_print = &section_src[..cmp::min(16, section_src.len())];
            println!("  loaded segment {:X?}", to_print);
        }
    }

    // remove writable flag for pages that dont want to be writable
    for (&page, &original_flags) in &pages_to_map {
        if !original_flags.contains(PageTableFlags::WRITABLE) {
            unsafe {
                mapper.update_flags(page, original_flags).unwrap().flush();
            }
        }
    } 

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
