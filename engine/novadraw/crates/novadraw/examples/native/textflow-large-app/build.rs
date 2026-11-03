use std::process::Command;

fn output(program: &str, args: &[&str]) -> String {
    Command::new(program)
        .args(args)
        .output()
        .ok()
        .filter(|result| result.status.success())
        .map(|result| String::from_utf8_lossy(&result.stdout).trim().to_owned())
        .unwrap_or_else(|| "unavailable".into())
}

fn main() {
    let rustc = std::env::var("RUSTC").unwrap_or_else(|_| "rustc".into());
    println!(
        "cargo:rustc-env=RESEARCH_RUSTC={}",
        output(&rustc, &["--version"])
    );
    println!(
        "cargo:rustc-env=RESEARCH_GIT={}",
        output("git", &["rev-parse", "HEAD"])
    );
    let status = output("git", &["status", "--porcelain"]);
    println!("cargo:rustc-env=RESEARCH_DIRTY={}", !status.is_empty());
    println!(
        "cargo:rustc-env=RESEARCH_PROFILE={}",
        std::env::var("PROFILE").unwrap()
    );
    // Capture metadata on rebuild; this is a build-time working-tree observation, not a hash
    // of every source byte and not the state of the tree when the application later runs.
}
