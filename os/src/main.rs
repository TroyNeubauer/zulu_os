extern crate anyhow;

use anyhow::{Context, Result};

fn main() -> Result<()> {
    // read env variables that were set in build script
    let uefi_path = env!("UEFI_PATH");
    let bios_path = env!("BIOS_PATH");

    // choose whether to start the UEFI or BIOS image
    let uefi = false;

    let mut cmd = std::process::Command::new("qemu-system-x86_64");
    cmd.args(&["-cpu", "Haswell-v1,+fsgsbase"])
        .args(&["-s", "-S"]);

    if uefi {
        cmd.arg("-bios").arg(ovmf_prebuilt::ovmf_pure_efi());
        cmd.arg("-drive")
            .arg(format!("format=raw,file={uefi_path}"));
    } else {
        cmd.arg("-drive")
            .arg(format!("format=raw,file={bios_path}"));
    }
    let mut child = cmd
        .spawn()
        .context("Failed to execute qemu-system-x86_64")?;

    let mut vnc = std::process::Command::new("vncviewer");
    let mut vnc = vnc.arg(":5900")
        .spawn()
        .context("Failed to execute qemu-system-x86_64")?;

    child.wait().context("Qemu failed")?;
    vnc.wait().context("Vnc failed")?;
    Ok(())
}
