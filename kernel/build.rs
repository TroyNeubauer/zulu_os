//use std::{io::Write, process::Command};

fn main() {
    /*
    println!("cargo:rerun-if-changed=../userspace_test/");
    std::env::set_var("REBUILD", format!("{:?}", std::time::Instant::now()));
    println!("cargo:rerun-if-env-changed=REBUILD");

    #[cfg(debug_assertions)]
    let args = ["build"];
    #[cfg(not(debug_assertions))]
    let args = ["build", "--release"];
    let output = Command::new("cargo")
        .args(args)
        .current_dir("../userspace_test/")
        .output()
        .expect("failed to execute process");
    if !output.status.success() {
        let _ = std::io::stderr().write(&output.stdout).unwrap();
        let _ = std::io::stderr().write(&output.stderr).unwrap();
        println!("cargo:rerun-if-changed=../userspace_test/");
        panic!("Failed to compile userspace test");
    }

    #[cfg(debug_assertions)]
    let profile = "debug";
    #[cfg(not(debug_assertions))]
    let profile = "release";
    Command::new("cp")
        .args([
            format!("target/x86_64/{profile}/userspace_test"),
            "../kernel/processes/".to_string(),
        ])
        .current_dir("../userspace_test/")
        .output()
        .expect("failed to execute process");
    */
}
