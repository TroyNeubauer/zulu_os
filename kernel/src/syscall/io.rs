use syscall::Result;

use crate::println;

pub fn write(_fd: usize, bytes: &[u8]) -> Result<usize> {
    println!("write");
    crate::vga_buffer::print_bytes(bytes);
    Ok(bytes.len())
}

pub fn read(_fd: usize, bytes: &mut [u8]) -> Result<usize> {
    println!("read");
    Ok(bytes.len())
}
