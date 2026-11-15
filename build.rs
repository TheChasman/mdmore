use std::process::Command;

// Stamp the short git commit into the binary as FROGRUST_COMMIT.
// Left unset (reported as "unknown") when git or the repo isn't available.
fn main() {
    let commit = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    if let Some(commit) = commit {
        println!("cargo:rustc-env=FROGRUST_COMMIT={commit}");
    }

    // Worktrees keep HEAD outside ./.git, so ask git where it lives.
    if let Some(head) = Command::new("git")
        .args(["rev-parse", "--git-path", "HEAD"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
    {
        println!("cargo:rerun-if-changed={}", head.trim());
    }
}
