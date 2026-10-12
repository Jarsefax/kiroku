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

fn without_ansi(text: &str) -> String {
    let mut plain = String::new();
    let mut chars = text.chars();
    while let Some(ch) = chars.next() {
        if ch == '\x1b' {
            assert_eq!(chars.next(), Some('['));
            for ch in chars.by_ref() {
                if ch == 'm' {
                    break;
                }
            }
        } else {
            plain.push(ch);
        }
    }
    plain
}

#[test]
fn colored_listings_preserve_text_references_and_stored_titles() {
    let temp = tempfile::tempdir().unwrap();
    let logs = temp.path();
    for (command, title) in [
        ("todo", "plan café"),
        ("doing", "möte 日本語"),
        ("done", "finished"),
    ] {
        let output = success(
            logs,
            &[command, title, "--date", "2026-10-06", "--color", "always"],
        );
        assert!(output.contains('\x1b'));
        assert!(output.contains(title));
    }
    let stored = fs::read(logs.join("2026-10.md")).unwrap();
    assert!(!stored.contains(&0x1b));
    for args in [
        &["list", "2026-10-06"][..],
        &["search", "möte", "2026-10-06"],
        &["list", "2026-10-06", "--status", "done"],
        &["list", "2026-09-01"],
    ] {
        let plain = success(logs, args);
        assert!(!plain.contains('\x1b'), "Piped output must be plain");
        let mut colored_args = args.to_vec();
        colored_args.extend(["--color", "always"]);
        let colored = success(logs, &colored_args);
        assert!(colored.contains('\x1b'));
        assert_eq!(without_ansi(&colored), plain);
        let mut never_args = args.to_vec();
        never_args.extend(["--color", "never"]);
        assert_eq!(success(logs, &never_args), plain);
    }
    let colored = success(logs, &["list", "2026-10-06", "--color", "always"]);
    for (code, label) in [("1;34", "To do"), ("1;33", "Doing"), ("1;32", "Done")] {
        assert!(colored.contains(&format!("\x1b[{code}m{label}\x1b[0m")));
    }
    assert_eq!(fs::read(logs.join("2026-10.md")).unwrap(), stored);
}

#[test]
fn explicit_color_overrides_environment_and_styles_runtime_errors() {
    let temp = tempfile::tempdir().unwrap();
    let logs = temp.path();
    let output = Command::new(env!("CARGO_BIN_EXE_krk"))
        .arg("--data-dir")
        .arg(logs)
        .args(["list", "--color", "always"])
        .env("NO_COLOR", "1")
        .env("TERM", "dumb")
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(output.stdout.contains(&0x1b));
    assert!(!success(logs, &["path", "--color", "always"]).contains('\x1b'));

    let plain = run(logs, &["done", "1", "--color", "never"]);
    let colored = run(logs, &["done", "1", "--color", "always"]);
    assert!(!plain.status.success());
    assert!(!colored.status.success());
    let stderr = String::from_utf8(colored.stderr).unwrap();
    assert!(stderr.contains("\x1b[1;31mkrk:\x1b[0m"));
    assert_eq!(
        without_ansi(&stderr),
        String::from_utf8(plain.stderr).unwrap()
    );
}

#[test]
fn first_title_addition_starts_today_for_each_status_command() {
    for (command, status) in [
        ("todo", Status::Todo),
        ("doing", Status::Doing),
        ("done", Status::Done),
    ] {
        let temp = tempfile::tempdir().unwrap();
        let logs = temp.path().join("logs");
        let output = success(&logs, &[command, "first item"]);
        assert!(output.contains("1. first item"));
        let store = Store::new(logs);
        let day = store.active().unwrap().unwrap();
        assert_eq!(day.date, Local::now().date_naive());
        assert_eq!(day.items.len(), 1);
        assert_eq!(day.items[0].status, status);
    }
}

#[test]
fn date_option_adds_all_statuses_and_keeps_older_dates_out_of_the_active_day() {
    let temp = tempfile::tempdir().unwrap();
    let logs = temp.path();
    let store = Store::new(logs.to_path_buf());
    let first = success(logs, &["todo", "weekly meeting", "--date", "2026-10-06"]);
    assert!(first.contains("Updated 2026-10-06."));
    assert!(first.contains("- weekly meeting"));
    success(logs, &["doing", "--date", "2026-10-06", "möte 日本語"]);
    success(logs, &["done", "backfill", "--date", "2026-09-30"]);
    let older = success(logs, &["todo", "earlier", "--date", "2026-10-02"]);
    assert!(!older.contains("active —"));
    assert!(!older.contains("Updated active day"));
    success(logs, &["todo", "default destination"]);
    let active = store.active().unwrap().unwrap();
    assert_eq!(active.date, parse_date("2026-10-06").unwrap());
    assert_eq!(active.items.len(), 3);
    assert_eq!(active.items[2].status, Status::Doing);
    let older = store
        .days(Some((
            parse_date("2026-09-30").unwrap(),
            parse_date("2026-09-30").unwrap(),
        )))
        .unwrap();
    assert_eq!(older[0].items[0].status, Status::Done);
    let future = (Local::now().date_naive() + chrono::Duration::days(40))
        .format("%Y-%m-%d")
        .to_string();
    success(logs, &["done", "future record", "--date", &future]);
    assert_eq!(
        store.active().unwrap().unwrap().date,
        parse_date("2026-10-06").unwrap()
    );
    assert!(success(logs, &["todo", "--help"]).contains("--date"));
}

#[test]
fn date_option_copies_active_items_historical_items_and_numbered_results() {
    let temp = tempfile::tempdir().unwrap();
    let logs = temp.path();
    let store = Store::new(logs.to_path_buf());
    success(logs, &["done", "source meeting", "--date", "2026-10-06"]);
    let source_path = logs.join("2026-10.md");
    let original = fs::read(&source_path).unwrap();
    success(
        logs,
        &["repeat", "1", "--date", "2026-09-30", "--status", "doing"],
    );
    success(
        logs,
        &[
            "repeat",
            "--from",
            "2026-10-06",
            "A",
            "--date",
            "2026-09-29",
        ],
    );
    // The explicit destination may precede or equal the source date.
    success(
        logs,
        &[
            "search",
            "source",
            "2026-10-06",
            "--repeat",
            "1",
            "--date",
            "2026-09-28",
        ],
    );
    success(
        logs,
        &[
            "list",
            "2026-10-06",
            "--status",
            "done",
            "--repeat",
            "1",
            "--date",
            "2026-09-27",
        ],
    );
    success(
        logs,
        &[
            "list",
            "2026-10-06",
            "--repeat",
            "1",
            "--date",
            "2026-09-26",
        ],
    );
    assert_eq!(fs::read(&source_path).unwrap(), original);
    let days = store.days(None).unwrap();
    assert_eq!(days.len(), 6);
    assert!(
        days.iter()
            .all(|day| day.items.len() == 1 && day.items[0].title == "source meeting")
    );
    assert_eq!(days[4].items[0].status, Status::Doing);
    assert_eq!(days[0].items[0].status, Status::Todo);
    let output = success(
        logs,
        &[
            "search",
            "source",
            "2026-10-06",
            "--repeat",
            "1",
            "--date",
            "2026-10-06",
        ],
    );
    assert!(output.contains("Updated 2026-10-06."));
    assert_eq!(store.active().unwrap().unwrap().items.len(), 2);
}

#[test]
fn future_dates_do_not_capture_default_additions_moves_copies_or_new_day() {
    let temp = tempfile::tempdir().unwrap();
    let logs = temp.path();
    let store = Store::new(logs.to_path_buf());
    let today = Local::now().date_naive();
    let yesterday = today - chrono::Duration::days(1);
    let future = today + chrono::Duration::days(1);
    let past_text = yesterday.format("%Y-%m-%d").to_string();
    let future_text = future.format("%Y-%m-%d").to_string();
    success(logs, &["todo", "earlier", "--date", &past_text]);
    success(logs, &["todo", "future plan", "--date", &future_text]);
    success(logs, &["todo", "default addition"]);
    success(logs, &["doing", "1"]);
    assert_eq!(store.active().unwrap().unwrap().date, yesterday);
    let listed = success(logs, &["list", &future_text]);
    assert!(listed.contains("future — letters"));
    assert!(!listed.contains("numbers are editable"));
    success(logs, &["search", "future plan", "--repeat", "1"]);
    let active = store.active().unwrap().unwrap();
    assert_eq!(active.date, yesterday);
    assert_eq!(active.items.len(), 3);
    assert!(
        active
            .items
            .iter()
            .any(|item| item.title == "earlier" && item.status == Status::Doing)
    );
    success(logs, &["new-day"]);
    success(logs, &["todo", "today addition"]);
    assert_eq!(store.active().unwrap().unwrap().date, today);
    assert_eq!(store.active().unwrap().unwrap().items.len(), 4);
    let future_day = store.days(Some((future, future))).unwrap().remove(0);
    assert_eq!(future_day.items.len(), 1);
    assert_eq!(future_day.items[0].title, "future plan");
    assert_eq!(future_day.items[0].status, Status::Todo);
}

#[test]
fn future_only_cli_starts_today_when_adding_a_title() {
    let temp = tempfile::tempdir().unwrap();
    let logs = temp.path();
    let today = Local::now().date_naive();
    let future = today + chrono::Duration::days(40);
    let future_text = future.format("%Y-%m-%d").to_string();
    success(logs, &["todo", "future plan", "--date", &future_text]);
    let path = logs.join(format!("{}.md", future.format("%Y-%m")));
    let original = fs::read(&path).unwrap();
    for args in [
        &["repeat", "1"][..],
        &["search", "future plan", "--repeat", "1"],
    ] {
        let result = run(logs, args);
        assert!(!result.status.success());
        assert!(String::from_utf8_lossy(&result.stderr).contains("new-day"));
        assert_eq!(fs::read(&path).unwrap(), original);
    }
    success(logs, &["todo", "default"]);
    let store = Store::new(logs.to_path_buf());
    assert_eq!(store.active().unwrap().unwrap().date, today);
    assert_eq!(store.active().unwrap().unwrap().items[0].title, "default");
    assert_eq!(fs::read(path).unwrap(), original);
}

#[test]
fn invalid_date_options_and_numbered_moves_leave_files_unchanged() {
    let temp = tempfile::tempdir().unwrap();
    let logs = temp.path();
    success(logs, &["todo", "source", "--date", "2026-10-06"]);
    let path = logs.join("2026-10.md");
    let original = fs::read(&path).unwrap();
    for args in [
        &["todo", "new", "--date", "2026-02-30"][..],
        &["doing", "new", "--date", "2026-1-2"],
        &["done", "new", "--date", "today"],
        &["todo", "1", "--date", "2026-09-30"],
        &["doing", "1", "--date", "2026-09-30"],
        &["done", "1", "--date", "2026-09-30"],
        &["edit", "1", "new", "--date", "2026-09-30"],
        &["remove", "1", "--date", "2026-09-30"],
        &["new-day", "--date", "2026-09-30"],
        &["list", "--date", "2026-09-30"],
        &["search", "source", "--date", "2026-09-30"],
        &["repeat", "2", "--date", "2026-09-30"],
        &["search", "source", "--repeat", "99", "--date", "2026-09-30"],
        &["todo", "", "--date", "2026-09-30"],
    ] {
        let result = run(logs, args);
        assert!(!result.status.success(), "{args:?}");
        assert_eq!(fs::read(&path).unwrap(), original, "{args:?}");
        assert!(!logs.join("2026-09.md").exists());
        assert!(!logs.join("backups").exists());
    }
    let error = run(logs, &["todo", "1", "--date", "2026-09-30"]);
    assert!(
        String::from_utf8_lossy(&error.stderr)
            .contains("numbered moves still apply to the active day")
    );
}

#[test]
fn storage_version_checks_reach_cli_errors_and_preserve_logs() {
    let temp = tempfile::tempdir().unwrap();
    let logs = temp.path();
    let path = logs.join("2026-10.md");
    let legacy = include_str!("fixtures/storage/legacy-v1.md");
    fs::write(&path, legacy).unwrap();
    assert!(success(logs, &["list", "2026-10-06"]).contains("plan café 日本語"));
    assert_eq!(fs::read_to_string(&path).unwrap(), legacy);
    success(logs, &["edit", "1", "updated title"]);
    let supported = fs::read_to_string(&path).unwrap();
    assert!(supported.starts_with("<!-- kiroku-format: 1 -->\n"));
    let backup_path = logs.join("backups/2026-10.md.000001.bak");
    assert_eq!(fs::read_to_string(&backup_path).unwrap(), legacy);

    let future = supported.replacen("kiroku-format: 1", "kiroku-format: 2", 1);
    fs::write(&path, &future).unwrap();
    for args in [
        &["list", "2026-10-06"][..],
        &["search", "updated"],
        &["todo", "new title"],
        &["new-day"],
        &["repeat", "1"],
    ] {
        let result = run(logs, args);
        assert!(!result.status.success(), "{args:?}");
        let error = String::from_utf8_lossy(&result.stderr);
        assert!(error.contains("2026-10.md:1"), "{error}");
        assert!(error.contains("Storage format 2 is unsupported"), "{error}");
        assert!(error.contains("Upgrade Kiroku"), "{error}");
        assert!(result.stdout.is_empty());
        assert_eq!(fs::read_to_string(&path).unwrap(), future);
        assert_eq!(fs::read_dir(logs.join("backups")).unwrap().count(), 1);
        assert_eq!(fs::read_to_string(&backup_path).unwrap(), legacy);
    }
    // App version reporting does not require opening or converting logs.
    assert_eq!(
        success(logs, &["--version"]),
        format!("krk {}\n", env!("CARGO_PKG_VERSION"))
    );
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
