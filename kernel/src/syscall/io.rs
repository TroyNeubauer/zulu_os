use syscall::Result;

pub fn write(_fd: usize, bytes: &[u8]) -> Result<usize> {
    crate::vga_buffer::print_bytes(bytes);
    Ok(bytes.len())
}

pub fn read(_fd: usize, bytes: &mut [u8]) -> Result<usize> {
    Ok(bytes.len())
}
