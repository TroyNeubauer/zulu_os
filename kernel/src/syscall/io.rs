use syscall::Result;

pub fn write(_fd: usize, bytes: &[u8]) -> Result<usize> {
    crate::vga_buffer::print_bytes(bytes);
    Ok(bytes.len())
}
