use chrono::{Datelike, Local};
use kiroku::storage::{Change, Status, Store, parse_date};
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
fn repeat_cli_copies_active_and_date_scoped_historical_items() {
    let temp = tempfile::tempdir().unwrap();
    let logs = temp.path();
    let store = Store::new(logs.to_path_buf());
    store.new_day(parse_date("2026-09-29").unwrap()).unwrap();
    store
        .change(Change::Add("older record".into(), Status::Done), None)
        .unwrap();
    store.new_day(parse_date("2026-09-30").unwrap()).unwrap();
    for n in 0..27 {
        let title = if n == 1 {
            "möte 日本語".into()
        } else if n == 26 {
            "123".into()
        } else {
            format!("completed {n}")
        };
        store
            .change(Change::Add(title, Status::Done), None)
            .unwrap();
    }
    store.new_day(parse_date("2026-10-02").unwrap()).unwrap();
    let path = logs.join("2026-09.md");
    let original = fs::read(&path).unwrap();
    let listing = success(logs, &["list", "2026-09-30"]);
    assert!(listing.contains("B. möte 日本語"));
    assert!(listing.contains("AA. 123"));
    // Labels come from a single-date listing, even when other history exists.
    let copied = success(logs, &["repeat", "--from", "2026-09-30", "B"]);
    assert!(copied.contains("1. möte 日本語"));
    assert!(copied.contains("Updated active day"));
    success(
        logs,
        &["repeat", "AA", "--from", "2026-09-30", "--status", "doing"],
    );
    success(logs, &["repeat", "1", "--status", "done"]);
    let day = store.active().unwrap().unwrap();
    assert_eq!(
        day.numbered()
            .iter()
            .map(|i| (i.title.as_str(), i.status))
            .collect::<Vec<_>>(),
        [
            ("möte 日本語", Status::Todo),
            ("123", Status::Doing),
            ("möte 日本語", Status::Done),
        ]
    );
    success(logs, &["repeat", "3"]);
    assert_eq!(store.active().unwrap().unwrap().items.len(), 4);
    assert_eq!(
        store.active().unwrap().unwrap().date,
        parse_date("2026-10-02").unwrap()
    );
    assert_eq!(fs::read(&path).unwrap(), original);
    assert!(success(logs, &["repeat", "--help"]).contains("--from"));
}

#[test]
fn invalid_repeat_arguments_leave_logs_unchanged_and_require_a_started_day() {
    let temp = tempfile::tempdir().unwrap();
    let logs = temp.path();
    let missing = run(logs, &["repeat", "1"]);
    assert!(!missing.status.success());
    assert!(String::from_utf8_lossy(&missing.stderr).contains("new-day"));
    let store = Store::new(logs.to_path_buf());
    store.new_day(parse_date("2026-09-30").unwrap()).unwrap();
    store
        .change(Change::Add("meeting".into(), Status::Done), None)
        .unwrap();
    store.new_day(parse_date("2026-10-02").unwrap()).unwrap();
    store
        .change(Change::Add("active item".into(), Status::Done), None)
        .unwrap();
    let source_path = logs.join("2026-09.md");
    let active_path = logs.join("2026-10.md");
    let source = fs::read(&source_path).unwrap();
    let active = fs::read(&active_path).unwrap();
    let backups = fs::read_dir(logs.join("backups")).unwrap().count();
    for args in [
        &["repeat"][..],
        &["repeat", "0"],
        &["repeat", "2"],
        &["repeat", "B"],
        &["repeat", "weekly meeting"],
        &["repeat", "+1"],
        &["repeat", "9999999999999999999999999999999999999999999"],
        &["repeat", "--from", "2026-09-30", "1"],
        &["repeat", "--from", "2026-09-30", "a"],
        &["repeat", "--from", "2026-09-30", "B"],
        &[
            "repeat",
            "--from",
            "2026-09-30",
            "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
        ],
        &["repeat", "--from", "2026-09-29", "A"],
        &["repeat", "--from", "2026-10-02", "A"],
        &["repeat", "--from", "2026-10-03", "A"],
        &["repeat", "--from", "2026-02-30", "A"],
        &["repeat", "1", "--status", "unknown"],
    ] {
        assert!(!run(logs, args).status.success(), "{args:?}");
        assert_eq!(fs::read(&source_path).unwrap(), source, "{args:?}");
        assert_eq!(fs::read(&active_path).unwrap(), active, "{args:?}");
        assert_eq!(
            fs::read_dir(logs.join("backups")).unwrap().count(),
            backups,
            "{args:?}"
        );
    }
}

#[test]
fn search_and_filtered_list_preserve_copyable_references_without_writing() {
    let temp = tempfile::tempdir().unwrap();
    let logs = temp.path();
    let store = Store::new(logs.to_path_buf());
    store.new_day(parse_date("2026-09-29").unwrap()).unwrap();
    store
        .change(Change::Add("unrelated".into(), Status::Done), None)
        .unwrap();
    store
        .change(Change::Add("möte 日本語".into(), Status::Done), None)
        .unwrap();
    store.new_day(parse_date("2026-09-30").unwrap()).unwrap();
    for n in 0..27 {
        let title = if n == 26 {
            "MÖTE 日本語".into()
        } else {
            format!("other {n}")
        };
        store
            .change(Change::Add(title, Status::Done), None)
            .unwrap();
    }
    store.new_day(parse_date("2026-10-02").unwrap()).unwrap();
    store
        .change(Change::Add("unrelated active".into(), Status::Todo), None)
        .unwrap();
    store
        .change(Change::Add("möte draft".into(), Status::Doing), None)
        .unwrap();
    store
        .change(Change::Add("möte 日本語".into(), Status::Done), None)
        .unwrap();
    let source_path = logs.join("2026-09.md");
    let active_path = logs.join("2026-10.md");
    let original_source = fs::read(&source_path).unwrap();
    let original_active = fs::read(&active_path).unwrap();
    let backups = fs::read_dir(logs.join("backups")).unwrap().count();
    let found = success(logs, &["search", "  MÖTE  ", "--status", "done"]);
    assert!(found.contains("3 matching items"));
    assert!(found.contains("2026-09-29"));
    assert!(found.contains("B. möte 日本語"));
    assert!(found.contains("AA. MÖTE 日本語"));
    assert!(found.contains("3. möte 日本語"));
    assert!(!found.contains("draft"));
    assert!(!found.contains("unrelated"));
    assert!(found.contains("krk repeat --from YYYY-MM-DD LETTER"));
    assert_eq!(
        success(logs, &["list", "--search", "  MÖTE  ", "--status", "done"]),
        found
    );
    let period = success(logs, &["search", "möte", "2026-09-30", "--status", "done"]);
    assert!(period.contains("1 matching item."));
    assert!(period.contains("AA. MÖTE 日本語"));
    assert!(!period.contains("B. möte"));
    assert_eq!(
        success(
            logs,
            &["list", "2026-09-30", "--search", "möte", "--status", "done"]
        ),
        period
    );
    let status_only = success(logs, &["list", "--status", "doing"]);
    assert!(status_only.contains("2. möte draft"));
    assert!(!status_only.contains("3. möte 日本語"));
    let all = success(logs, &["search", ""]);
    assert!(all.contains("32 matching items"));
    let none = success(logs, &["search", "absent"]);
    assert!(none.contains("No matching items"));
    assert!(!none.contains("krk repeat NUMBER"));
    for args in [
        &["search"][..],
        &["search", "möte", "--status", "invalid"],
        &["search", "möte", "2026-02-30"],
        &["search", "bad\ntext"],
        &["list", "--search", "\u{1b}[2J"],
    ] {
        assert!(!run(logs, args).status.success(), "{args:?}");
    }
    assert_eq!(fs::read(&source_path).unwrap(), original_source);
    assert_eq!(fs::read(&active_path).unwrap(), original_active);
    assert_eq!(fs::read_dir(logs.join("backups")).unwrap().count(), backups);
    // These are references from the search output, not positions among matches.
    success(logs, &["repeat", "--from", "2026-09-30", "AA"]);
    let day = store.active().unwrap().unwrap();
    assert_eq!(day.numbered()[1].title, "MÖTE 日本語");
    assert_eq!(day.numbered()[1].status, Status::Todo);
    assert_eq!(fs::read(&source_path).unwrap(), original_source);
    assert!(success(logs, &["search", "--help"]).contains("--status"));
    assert!(success(logs, &["list", "--help"]).contains("--search"));
}

#[test]
fn searching_does_not_start_today_or_create_missing_directories() {
    let temp = tempfile::tempdir().unwrap();
    let logs = temp.path().join("missing");
    assert!(success(&logs, &["search", "meeting"]).contains("No matching items"));
    assert!(success(&logs, &["list", "--status", "done"]).contains("No matching items"));
    assert!(!logs.exists());
}

#[test]
fn repeat_option_copies_search_and_list_results_into_todo() {
    for (args, result, title) in [
        (&["search", "meeting"][..], "3", "finished meeting"),
        (
            &["search", "meeting", "--status", "done"][..],
            "2",
            "finished meeting",
        ),
        (&["list", "--search", "meeting"][..], "1", "old meeting"),
        (
            &["list", "2026-10-01", "--search", "meeting"][..],
            "1",
            "second meeting",
        ),
        (
            &["list", "2026-09-29..2026-10-02"][..],
            "5",
            "finished meeting",
        ),
        (&["list"][..], "3", "current meeting"),
    ] {
        let temp = tempfile::tempdir().unwrap();
        let logs = temp.path();
        let store = Store::new(logs.to_path_buf());
        store.new_day(parse_date("2026-09-29").unwrap()).unwrap();
        store
            .change(Change::Add("unrelated".into(), Status::Todo), None)
            .unwrap();
        store
            .change(Change::Add("old meeting".into(), Status::Done), None)
            .unwrap();
        store.new_day(parse_date("2026-10-01").unwrap()).unwrap();
        store
            .change(Change::Add("second meeting".into(), Status::Doing), None)
            .unwrap();
        store
            .change(Change::Add("finished meeting".into(), Status::Done), None)
            .unwrap();
        store.new_day(parse_date("2026-10-02").unwrap()).unwrap();
        store
            .change(Change::Add("current meeting".into(), Status::Doing), None)
            .unwrap();
        let source_bytes = fs::read(logs.join("2026-09.md")).unwrap();
        let historical = |store: &Store| {
            store
                .days(Some((
                    parse_date("2026-09-29").unwrap(),
                    parse_date("2026-10-01").unwrap(),
                )))
                .unwrap()
                .into_iter()
                .map(|day| {
                    (
                        day.date,
                        day.items
                            .into_iter()
                            .map(|i| (i.title, i.status))
                            .collect::<Vec<_>>(),
                    )
                })
                .collect::<Vec<_>>()
        };
        let history = historical(&store);
        let before = store.active().unwrap().unwrap();
        let todo_count = before
            .numbered()
            .iter()
            .filter(|i| i.status == Status::Todo)
            .count();
        let listing = success(logs, args);
        assert!(
            listing.lines().any(
                |line| line.trim_start().starts_with(&format!("[{result}] "))
                    && line.ends_with(title)
            ),
            "{args:?}: {listing}"
        );
        assert!(listing.contains("Append `--repeat N`"));
        let mut with_repeat = args.to_vec();
        with_repeat.extend(["--repeat", result]);
        let output = success(logs, &with_repeat);
        assert!(output.contains(&format!("Repeated result {result} from")));
        assert!(output.contains("Updated active day"));
        let after = store.active().unwrap().unwrap();
        assert_eq!(after.date, before.date);
        assert_eq!(after.items.len(), before.items.len() + 1);
        assert_eq!(after.numbered()[todo_count].title, title);
        assert_eq!(after.numbered()[todo_count].status, Status::Todo);
        for original in &before.items {
            assert!(
                after
                    .items
                    .iter()
                    .any(|i| i.title == original.title && i.status == original.status)
            );
        }
        assert_eq!(historical(&store), history);
        assert_eq!(fs::read(logs.join("2026-09.md")).unwrap(), source_bytes);
    }
}

#[test]
fn invalid_repeat_option_does_not_write_or_repeat_all_matches() {
    let temp = tempfile::tempdir().unwrap();
    let logs = temp.path();
    let store = Store::new(logs.to_path_buf());
    store.new_day(parse_date("2026-10-02").unwrap()).unwrap();
    store
        .change(Change::Add("möte 日本語".into(), Status::Done), None)
        .unwrap();
    let path = logs.join("2026-10.md");
    let before = fs::read(&path).unwrap();
    let backups = fs::read_dir(logs.join("backups")).unwrap().count();
    for args in [
        &["search", "möte", "--repeat"][..],
        &["search", "möte", "--repeat", "0"],
        &["search", "möte", "--repeat", "2"],
        &["search", "möte", "--repeat", "A"],
        &[
            "search",
            "möte",
            "--repeat",
            "9999999999999999999999999999999999999",
        ],
        &["search", "absent", "--repeat", "1"],
        &["search", "möte", "--status", "todo", "--repeat", "1"],
        &["list", "--repeat", "0"],
        &["list", "2026-10-01", "--repeat", "1"],
    ] {
        assert!(!run(logs, args).status.success(), "{args:?}");
        assert_eq!(fs::read(&path).unwrap(), before, "{args:?}");
        assert_eq!(fs::read_dir(logs.join("backups")).unwrap().count(), backups);
    }
    success(logs, &["search", "MÖTE", "--repeat", "1"]);
    let day = store.active().unwrap().unwrap();
    assert_eq!(day.numbered()[0].title, "möte 日本語");
    assert_eq!(day.numbered()[0].status, Status::Todo);
    assert_eq!(day.numbered()[1].status, Status::Done);
    assert!(success(logs, &["search", "--help"]).contains("--repeat"));
    assert!(success(logs, &["list", "--help"]).contains("--repeat"));
    let missing = temp.path().join("no recorded day");
    assert!(
        !run(&missing, &["search", "möte", "--repeat", "1"])
            .status
            .success()
    );
    assert!(!missing.exists());
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
        &["done", "0"][..],
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

#[test]
fn every_status_command_adds_titles_and_moves_existing_items() {
    for (command, status) in [
        ("todo", Status::Todo),
        ("doing", Status::Doing),
        ("done", Status::Done),
    ] {
        let temp = tempfile::tempdir().unwrap();
        let logs = temp.path();
        assert!(!run(logs, &[command, "meeting"]).status.success());
        success(logs, &["new-day"]);
        success(logs, &[command, "möte 日本語 123"]);
        let store = Store::new(logs.to_path_buf());
        let day = store.active().unwrap().unwrap();
        assert_eq!(day.items.len(), 1);
        assert_eq!(day.items[0].title, "möte 日本語 123");
        assert_eq!(day.items[0].status, status);

        for source in ["todo", "doing", "done"] {
            success(logs, &[source, "1"]);
            success(logs, &[command, "1"]);
            let day = store.active().unwrap().unwrap();
            assert_eq!(day.items.len(), 1);
            assert_eq!(day.items[0].title, "möte 日本語 123");
            assert_eq!(day.items[0].status, status);
        }

        success(logs, &[command, "möte 日本語 123"]);
        success(logs, &[command, "A"]);
        let day = store.active().unwrap().unwrap();
        assert_eq!(day.items.len(), 3);
        assert!(day.items.iter().all(|item| item.status == status));
        assert_eq!(day.items[1].title, "möte 日本語 123");
        assert_eq!(day.items[2].title, "A");

        let path = logs.join(format!("{}.md", Local::now().format("%Y-%m")));
        let before = fs::read(&path).unwrap();
        for value in [
            "",
            "  ",
            "bad\ntitle",
            "0",
            "4",
            "9999999999999999999999999999999999999999",
        ] {
            assert!(
                !run(logs, &[command, value]).status.success(),
                "{command} {value:?}"
            );
            assert_eq!(fs::read(&path).unwrap(), before);
        }
        assert!(!run(logs, &[command]).status.success());
        assert_eq!(fs::read(&path).unwrap(), before);
        assert!(success(logs, &[command, "--help"]).contains("TITLE_OR_NUMBER"));
    }
}

#[test]
fn previous_month_edit_commands_back_up_original_or_abort_if_backup_unavailable() {
    let previous_month = Local::now()
        .date_naive()
        .with_day(1)
        .unwrap()
        .pred_opt()
        .unwrap();
    let name = format!("{}.md", previous_month.format("%Y-%m"));
    for args in [
        &["todo", "new todo"][..],
        &["doing", "new doing"],
        &["done", "new done"],
        &["todo", "1"],
        &["doing", "1"],
        &["done", "1"],
        &["edit", "1", "renamed"],
        &["remove", "1"],
        &["repeat", "1"],
    ] {
        for backup_unavailable in [false, true] {
            let temp = tempfile::tempdir().unwrap();
            let logs = temp.path();
            let path = logs.join(&name);
            let (todo, doing) = if args == ["todo", "1"] {
                ("", "- existing item\r\n")
            } else {
                ("- existing item\r\n", "")
            };
            let original = format!(
                "<!-- preserve exact bytes -->\r\n# {previous_month}\r\n\r\n## To do\r\n{todo}\r\n## Doing\r\n{doing}\r\n## Done\r\n"
            );
            fs::write(&path, &original).unwrap();
            if backup_unavailable {
                fs::write(logs.join("backups"), "not a directory").unwrap();
                let result = run(logs, args);
                assert!(!result.status.success(), "{args:?}");
                assert!(
                    String::from_utf8_lossy(&result.stderr)
                        .contains("Save aborted before replacement"),
                    "{args:?}: {}",
                    String::from_utf8_lossy(&result.stderr)
                );
                assert_eq!(fs::read(&path).unwrap(), original.as_bytes(), "{args:?}");
            } else {
                success(logs, args);
                assert_eq!(
                    fs::read(logs.join("backups").join(format!("{name}.000001.bak"))).unwrap(),
                    original.as_bytes(),
                    "{args:?}"
                );
                assert_ne!(fs::read(&path).unwrap(), original.as_bytes(), "{args:?}");
            }
            assert_eq!(
                Store::new(logs.to_path_buf())
                    .active()
                    .unwrap()
                    .unwrap()
                    .date,
                previous_month
            );
        }
    }
}
