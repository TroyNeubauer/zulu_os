use std::{io::Write, path::PathBuf, process::Command};

fn main() {
    println!("cargo:rerun-if-changed=../kernel/");
    std::env::set_var("REBUILD", format!("{:?}", std::time::Instant::now()));
    println!("cargo:rerun-if-env-changed=REBUILD");

    #[cfg(debug_assertions)]
    let args = ["build"];
    #[cfg(not(debug_assertions))]
    let args = ["build", "--release"];
    let output = Command::new("cargo")
        .args(args)
        .current_dir("../kernel/")
        .output()
        .expect("failed to execute process");

    if !output.status.success() {
        let _ = std::io::stderr().write(&output.stdout).unwrap();
        let _ = std::io::stderr().write(&output.stderr).unwrap();
        println!("cargo:rerun-if-changed=../kernel/");
        panic!("Failed to compile kernel");
    }

    #[cfg(debug_assertions)]
    let profile = "debug";
    #[cfg(not(debug_assertions))]
    let profile = "release";

    let kernel = PathBuf::from(format!(
        "../kernel/target/x86_64-unknown-none/{profile}/zulu_os"
    ));

    let out_dir = PathBuf::from(std::env::var_os("OUT_DIR").unwrap());

    // create an UEFI disk image (optional)
    let uefi_path = out_dir.join("uefi.img");
    bootloader::UefiBoot::new(&kernel)
        .create_disk_image(&uefi_path)
        .unwrap();

    // create a BIOS disk image
    let bios_path = out_dir.join("bios.img");
    bootloader::BiosBoot::new(&kernel)
        .create_disk_image(&bios_path)
        .unwrap();

    // pass the disk image paths as env variables to the `main.rs`
    println!("cargo:rustc-env=UEFI_PATH={}", uefi_path.display());
    println!("cargo:rustc-env=BIOS_PATH={}", bios_path.display());
}
