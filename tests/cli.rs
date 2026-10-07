use chrono::Local;
use std::{
    fs,
    process::{Command, Output},
};

fn run(directory: &std::path::Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_krk"))
        .arg("--data-dir")
        .arg(directory)
        .args(args)
        .output()
        .unwrap()
}
fn success(directory: &std::path::Path, args: &[&str]) -> String {
    let result = run(directory, args);
    assert!(
        result.status.success(),
        "{:?}: {}",
        args,
        String::from_utf8_lossy(&result.stderr)
    );
    String::from_utf8(result.stdout).unwrap()
}

#[test]
fn real_cli_workflow_with_unicode_duplicate_titles_and_errors() {
    let temp = tempfile::tempdir().unwrap();
    let logs = temp.path().join("log directory with spaces");
    assert_eq!(success(&logs, &["path"]).trim(), logs.to_str().unwrap());
    assert!(!logs.exists());
    assert!(!run(&logs, &["add", "meeting"]).status.success());
    success(&logs, &["new-day"]);
    success(&logs, &["add", "monthly op meeting - september"]);
    success(&logs, &["add", "weekly meeting"]);
    success(&logs, &["add", "weekly meeting"]);
    success(&logs, &["doing", "3"]);
    success(&logs, &["edit", "3", "möte 日本語"]);
    success(&logs, &["done", "3"]);
    let listing = success(&logs, &["list", "today"]);
    assert!(listing.contains("3. möte 日本語"));
    let path = logs.join(format!("{}.md", Local::now().format("%Y-%m")));
    let before = fs::read(&path).unwrap();
    for args in [
        &["done", "A"][..],
        &["remove", "0"],
        &["list", "2026-02-30"],
        &["add", "  "],
    ] {
        assert!(!run(&logs, args).status.success());
        assert_eq!(fs::read(&path).unwrap(), before);
    }
    assert!(success(&logs, &["new-day"]).contains("already started"));
    success(&logs, &["todo", "3"]);
    success(&logs, &["remove", "2"]);
    assert!(!success(&logs, &["list"]).contains("weekly meeting"));
    assert!(success(&logs, &["--help"]).contains("new-day"));
    let tui = run(&logs, &[]);
    assert!(!tui.status.success());
    assert!(String::from_utf8_lossy(&tui.stderr).contains("interactive terminal"));
}
