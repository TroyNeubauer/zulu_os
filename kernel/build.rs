use std::process::Command;

fn main() {
    #[cfg(debug_assertions)]
    let args = ["build"];
    #[cfg(not(debug_assertions))]
    let args = ["build", "--release"];
    let output = Command::new("cargo")
        .args(args)
        .current_dir("../userspace_test/")
        .output()
        .expect("failed to execute process");
    assert!(output.status.success());

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
    
}
