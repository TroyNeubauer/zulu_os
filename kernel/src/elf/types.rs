use {
    alloc::vec::Vec,
    core::ops::Range,
    num_enum::TryFromPrimitive,
    object::elf::*,
    x86_64::{
        structures::paging::{page::PageRangeInclusive, Page, PageTableFlags},
        VirtAddr,
    },
};

pub struct ElfFile {
    pub segments: Vec<ElfSegment>,
    pub entry_point: VirtAddr,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, TryFromPrimitive)]
#[repr(u32)]
pub enum ElfSegmentType {
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
pub struct VirtualMapDestination {
    /// Virtual address where this segment will be mapped into memory.
    /// Starts with the same value as `original_start`, but can be changed.
    pub start: VirtAddr,
    /// The length of this virtual addr segment in bytes
    pub len: usize,
    /// The original virtual address the segment file wanted
    pub original_start: u64,
}

#[derive(Debug, Clone)]
pub struct ElfSegment {
    pub ty: ElfSegmentType,
    pub align: usize,
    pub flags: PageTableFlags,
    /// Offset into the elf file where this segment data resides
    pub file_range: Range<usize>,
    pub addr: VirtualMapDestination,
}

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
    /// Returns the pages used by thes section, may overlap with other sections depending on
    /// alignment
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
