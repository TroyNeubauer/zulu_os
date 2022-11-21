mod types;
pub use types::*;

use alloc::{collections::BTreeMap, vec::Vec};
use core::{cmp, mem, slice};
use object::{
    elf::FileHeader64,
    read::elf::{FileHeader, ProgramHeader},
    LittleEndian,
};
use types::ElfFile;
use x86_64::{
    structures::paging::{FrameAllocator, Mapper, OffsetPageTable, Page, PageTableFlags, Size4KiB},
    VirtAddr,
};

use crate::println;

// syncs up with constant in gdb.sh so gdb knows where to look when were debugging this
const LOAD_TEXT_SECTION_AT: u64 = 0x660000;

pub fn load(
    bytes: &[u8],
    mapper: &mut OffsetPageTable<'static>,
    allocator: &mut impl FrameAllocator<Size4KiB>,
) -> ElfFile {
    let elf = FileHeader64::<LittleEndian>::parse(bytes).unwrap();
    let program_headers = elf.program_headers(LittleEndian, bytes).unwrap();

    println!("entry: 0x{:X?}", elf.e_entry);
    println!("type: {:?}", elf.e_type);

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
        let frame = allocator.allocate_frame().unwrap();
        // unconditionally add writable flag so we can copy the section data to it
        let flags = original_flags | PageTableFlags::WRITABLE;

        println!("  mapping {:?} to {:?} with {:?}", page, frame, flags);
        unsafe {
            mapper
                .map_to(page, frame, flags, allocator)
                .unwrap()
                .flush();
        };
    }

    for section in &elf_file.segments {
        if section.ty == ElfSegmentType::Load {
            let section_src = &bytes[section.file_range.clone()];
            let section_dst: &mut [u8] = unsafe {
                slice::from_raw_parts_mut(section.addr.start.as_mut_ptr(), section.file_range.len())
            };
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
    println!("Jumping to addr: {:?}", entry_point);
    println!("");
    unsafe { jmp(entry_point) }
    println!("");

    elf_file
}

#[no_mangle]
#[inline(never)]
pub unsafe extern "C" fn jmp(addr: *const u8) {
    let fn_ptr: fn() = unsafe { mem::transmute(addr) };
    fn_ptr();
}
