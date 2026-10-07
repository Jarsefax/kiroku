use chrono::Local;
use kiroku::storage::{Status, Store};
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
    assert!(!run(&logs, &["todo", "meeting"]).status.success());
    success(&logs, &["new-day"]);
    success(&logs, &["todo", "monthly op meeting - september"]);
    success(&logs, &["todo", "weekly meeting"]);
    success(&logs, &["todo", "weekly meeting"]);
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
        &["todo", "  "],
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

#[test]
fn todo_adds_text_and_moves_references_from_doing_and_done() {
    let temp = tempfile::tempdir().unwrap();
    let logs = temp.path();
    success(logs, &["new-day"]);
    success(logs, &["todo", "review 123"]);
    success(logs, &["todo", "möte 日本語"]);
    success(logs, &["doing", "1"]);
    success(logs, &["todo", "2"]);
    success(logs, &["done", "2"]);
    success(logs, &["todo", "2"]);
    let path = logs.join(format!("{}.md", Local::now().format("%Y-%m")));
    let before = fs::read(&path).unwrap();
    let day = Store::new(logs.to_path_buf()).active().unwrap().unwrap();
    assert_eq!(
        day.numbered()
            .into_iter()
            .map(|item| (item.title.as_str(), item.status))
            .collect::<Vec<_>>(),
        [("möte 日本語", Status::Todo), ("review 123", Status::Todo)]
    );
    for args in [
        &["add", "removed command"][..],
        &["todo"],
        &["todo", ""],
        &["todo", "0"],
        &["todo", "3"],
        &["todo", "9999999999999999999999999999999999999999"],
    ] {
        assert!(!run(logs, args).status.success(), "{args:?}");
        assert_eq!(fs::read(&path).unwrap(), before);
    }
    let help = success(logs, &["--help"]);
    assert!(help.contains("todo"));
    assert!(
        !help
            .lines()
            .any(|line| line.trim_start().starts_with("add "))
    );
    assert!(success(logs, &["todo", "--help"]).contains("TITLE_OR_NUMBER"));
}
