use std::process::Command;

fn run(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_frogrust"))
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .env_remove("FROGRUST_THEME")
        .args(args)
        .output()
        .unwrap()
}

#[test]
fn unknown_theme_exits_2() {
    let out = run(&["--theme", "no-such-theme", "TEST.md"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&out.stderr).contains("unknown theme 'no-such-theme'"));
}

#[test]
fn list_themes_prints_names() {
    let out = run(&["--list-themes"]);
    assert!(out.status.success());
    assert!(String::from_utf8_lossy(&out.stdout).lines().any(|l| l == "ansi"));
}

#[test]
fn known_theme_prints_when_piped() {
    let out = run(&["--theme", "Dracula", "TEST.md"]);
    assert!(out.status.success());
}
