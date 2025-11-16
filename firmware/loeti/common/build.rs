use std::process::Command;

/// Get current Git commit hash.
fn get_git_hash() {
    let git_hash = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|_| "unknown".into());

    // Pass the hash to the compiler as an environment variable
    println!("cargo:rustc-env=GIT_HASH={}", git_hash);
}

fn main() {
    get_git_hash();
}
