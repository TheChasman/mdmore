use std::process::Command;

fn git(args: &[&str]) -> Option<String> {
    let out = Command::new("git").args(args).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8(out.stdout).ok()?.trim().to_string();
    (!s.is_empty()).then_some(s)
}

// Stamp the short git commit into the binary as MDMORE_COMMIT.
// Left unset (reported as "unknown") when git or the repo isn't available.
fn main() {
    if let Some(commit) = git(&["rev-parse", "--short", "HEAD"]) {
        println!("cargo:rustc-env=MDMORE_COMMIT={commit}");
    }

    // Rebuild when the commit moves. HEAD only changes on checkout; a new commit
    // on a branch updates the branch ref (or packed-refs) instead. --git-path
    // resolves these correctly inside worktrees too.
    let mut watch = vec!["HEAD".to_string(), "packed-refs".to_string()];
    if let Some(branch_ref) = git(&["symbolic-ref", "-q", "HEAD"]) {
        watch.push(branch_ref);
    }
    for name in watch {
        if let Some(path) = git(&["rev-parse", "--git-path", &name]) {
            println!("cargo:rerun-if-changed={path}");
        }
    }
}
