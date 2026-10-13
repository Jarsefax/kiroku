use chrono::Local;
use kiroku::storage::{Status, Store, parse_date};
use std::{fs, path::Path, process::Command};

fn run(logs: &Path, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_krk"))
        .arg("--data-dir")
        .arg(logs)
        .args(args)
        .output()
        .unwrap()
}

fn success(logs: &Path, args: &[&str]) -> String {
    let output = run(logs, args);
    assert!(
        output.status.success(),
        "{args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

fn fixture() -> (tempfile::TempDir, Store) {
    let temp = tempfile::tempdir().unwrap();
    fs::write(temp.path().join("2026-10.md"), "# 2026-10-02\n\n## To do\n- A\n- B\n<!-- keep -->\n- C\n- D\n- E\n\n## Doing\n- F\n- G\n\n## Done\n- H\n- I\n").unwrap();
    let store = Store::new(temp.path().to_path_buf());
    (temp, store)
}

fn titles(store: &Store, date: &str, status: Status) -> Vec<String> {
    let date = parse_date(date).unwrap();
    store.days(Some((date, date))).unwrap()[0]
        .numbered()
        .into_iter()
        .filter(|item| item.status == status)
        .map(|item| item.title.clone())
        .collect()
}

#[test]
fn removals_use_original_numbers_and_deduplicate_overlapping_ranges() {
    for (selection, expected) in [
        ("1-3", vec!["D", "E"]),
        ("1,3", vec!["B", "D", "E"]),
        ("3,1-2,2-3", vec!["D", "E"]),
        ("1-3,5-7", vec!["D"]),
    ] {
        let (_temp, store) = fixture();
        let original = fs::read(store.dir.join("2026-10.md")).unwrap();
        success(&store.dir, &["remove", selection]);
        assert_eq!(titles(&store, "2026-10-02", Status::Todo), expected);
        assert_eq!(titles(&store, "2026-10-02", Status::Done), ["H", "I"]);
        assert!(
            fs::read_to_string(store.dir.join("2026-10.md"))
                .unwrap()
                .contains("<!-- keep -->")
        );
        assert_eq!(fs::read_dir(store.dir.join("backups")).unwrap().count(), 1);
        assert_eq!(
            fs::read(store.dir.join("backups/2026-10.md.000001.bak")).unwrap(),
            original
        );
    }
}

#[test]
fn selected_moves_preserve_order_and_clamp_without_crossing_sections() {
    for (selection, direction, steps, expected) in [
        ("2-3", "up", "1", vec!["B", "C", "A", "D", "E"]),
        ("2-3", "down", "1", vec!["A", "D", "B", "C", "E"]),
        ("1,3", "down", "2", vec!["B", "D", "A", "E", "C"]),
        ("4,2", "up", "2", vec!["B", "D", "A", "C", "E"]),
        ("3-5", "down", "999", vec!["A", "B", "C", "D", "E"]),
        ("2-3,6-7", "up", "1", vec!["B", "C", "A", "D", "E"]),
    ] {
        let (_temp, store) = fixture();
        success(
            &store.dir,
            &["move", selection, direction, "--steps", steps],
        );
        assert_eq!(titles(&store, "2026-10-02", Status::Todo), expected);
        assert_eq!(titles(&store, "2026-10-02", Status::Doing), ["F", "G"]);
        assert_eq!(titles(&store, "2026-10-02", Status::Done), ["H", "I"]);
        assert!(
            fs::read_to_string(store.dir.join("2026-10.md"))
                .unwrap()
                .contains("<!-- keep -->\n")
        );
    }
}

#[test]
fn status_changes_and_edits_apply_to_the_whole_original_selection() {
    let (_temp, store) = fixture();
    success(&store.dir, &["done", "8,2,6", "--position", "1"]);
    assert_eq!(
        titles(&store, "2026-10-02", Status::Todo),
        ["A", "C", "D", "E"]
    );
    assert_eq!(titles(&store, "2026-10-02", Status::Doing), ["G"]);
    assert_eq!(
        titles(&store, "2026-10-02", Status::Done),
        ["B", "F", "H", "I"]
    );
    success(&store.dir, &["edit", "1,3", "same title 日本語"]);
    assert_eq!(
        titles(&store, "2026-10-02", Status::Todo),
        ["same title 日本語", "C", "same title 日本語", "E"]
    );
    success(&store.dir, &["todo", "6-8"]);
    assert_eq!(
        titles(&store, "2026-10-02", Status::Todo),
        [
            "same title 日本語",
            "C",
            "same title 日本語",
            "E",
            "B",
            "F",
            "H"
        ]
    );
    success(&store.dir, &["doing", "1-2"]);
    assert_eq!(
        titles(&store, "2026-10-02", Status::Doing),
        ["G", "same title 日本語", "C"]
    );
    success(&store.dir, &["todo", "meeting, follow-up"]);
    assert!(titles(&store, "2026-10-02", Status::Todo).contains(&"meeting, follow-up".to_owned()));
}

#[test]
fn dated_moves_and_repeats_insert_selected_items_as_an_ordered_block() {
    let (_temp, store) = fixture();
    success(
        &store.dir,
        &["doing", "1,3", "--date", "2026-09-30", "--position", "1"],
    );
    assert_eq!(titles(&store, "2026-09-30", Status::Doing), ["A", "C"]);
    assert_eq!(titles(&store, "2026-10-02", Status::Todo), ["B", "D", "E"]);
    let original = fs::read(store.dir.join("2026-09.md")).unwrap();
    success(
        &store.dir,
        &["repeat", "A-B,A", "--from", "2026-09-30", "--position", "2"],
    );
    assert_eq!(
        titles(&store, "2026-10-02", Status::Todo),
        ["B", "A", "C", "D", "E"]
    );
    assert_eq!(fs::read(store.dir.join("2026-09.md")).unwrap(), original);
    success(
        &store.dir,
        &[
            "done",
            "A-B",
            "--from",
            "2026-09-30",
            "--date",
            "2026-10-02",
        ],
    );
    assert!(titles(&store, "2026-09-30", Status::Doing).is_empty());
    assert_eq!(
        titles(&store, "2026-10-02", Status::Done),
        ["H", "I", "A", "C"]
    );
    success(
        &store.dir,
        &[
            "repeat",
            "1-2,2",
            "--date",
            "2026-09-29",
            "--status",
            "doing",
        ],
    );
    assert_eq!(titles(&store, "2026-09-29", Status::Doing), ["B", "A"]);
    success(&store.dir, &["repeat", "2,1", "--position", "1"]);
    assert_eq!(
        titles(&store, "2026-10-02", Status::Todo),
        ["B", "A", "B", "A", "C", "D", "E"]
    );
}

fn result_fixture() -> (tempfile::TempDir, Store) {
    let temp = tempfile::tempdir().unwrap();
    for (month, date, titles) in [
        ("2026-09", "2026-09-30", "- duplicate\n- historical\n"),
        ("2026-10", "2026-10-02", "- duplicate\n- finished\n"),
    ] {
        fs::write(
            temp.path().join(format!("{month}.md")),
            format!("# {date}\n\n## To do\n- keep\n\n## Doing\n\n## Done\n{titles}"),
        )
        .unwrap();
    }
    let store = Store::new(temp.path().to_path_buf());
    (temp, store)
}

#[test]
fn result_removals_and_copies_span_months_and_keep_display_order() {
    let (_temp, store) = result_fixture();
    let output = success(
        &store.dir,
        &["list", "--status", "done", "--remove", "3,1,1"],
    );
    assert!(output.contains("Removed result 1 from 2026-09-30: duplicate"));
    assert!(output.contains("Removed result 3 from 2026-10-02: duplicate"));
    assert_eq!(titles(&store, "2026-09-30", Status::Done), ["historical"]);
    assert_eq!(titles(&store, "2026-10-02", Status::Done), ["finished"]);
    assert!(output.contains("[1] historical"));
    assert!(output.contains("[2] finished"));
    assert_eq!(fs::read_dir(store.dir.join("backups")).unwrap().count(), 2);
    success(
        &store.dir,
        &[
            "search",
            "",
            "--status",
            "done",
            "--repeat",
            "1-2",
            "--position",
            "1",
        ],
    );
    assert_eq!(
        titles(&store, "2026-10-02", Status::Todo),
        ["historical", "finished", "keep"]
    );
    assert_eq!(titles(&store, "2026-10-02", Status::Done), ["finished"]);
    let output = success(
        &store.dir,
        &["search", "", "--status", "done", "--remove", "1-2"],
    );
    assert!(output.contains("No matching items."));
}

#[test]
fn invalid_selections_and_positions_do_not_apply_a_partial_batch() {
    let (_temp, store) = fixture();
    let path = store.dir.join("2026-10.md");
    let original = fs::read(&path).unwrap();
    let oversized = format!("1-{}", usize::MAX);
    for selection in [
        "1,99",
        "0-3",
        "3-1",
        "1,,3",
        "1-",
        "1--3",
        "1,",
        oversized.as_str(),
    ] {
        for args in [
            vec!["remove", selection],
            vec!["move", selection, "down"],
            vec!["edit", selection, "changed"],
            vec!["doing", selection],
            vec!["repeat", selection, "--date", "2026-09-30"],
            vec!["list", "--remove", selection],
            vec!["search", "", "--repeat", selection],
        ] {
            assert!(!run(&store.dir, &args).status.success(), "{args:?}");
            assert_eq!(fs::read(&path).unwrap(), original);
            assert!(!store.dir.join("2026-09.md").exists());
            assert!(!store.dir.join("backups").exists());
        }
    }
    for args in [
        &["doing", "1-3", "--position", "9"][..],
        &["todo", "1-3", "--position", "4"],
        &["repeat", "1-3", "--position", "9"],
        &["repeat", "1-3", "--date", "2026-09-30", "--position", "2"],
    ] {
        assert!(!run(&store.dir, args).status.success(), "{args:?}");
        assert_eq!(fs::read(&path).unwrap(), original);
        assert!(!store.dir.join("2026-09.md").exists());
        assert!(!store.dir.join("backups").exists());
    }
}

#[test]
fn copying_multiple_future_results_does_not_require_an_active_day_with_a_destination() {
    let temp = tempfile::tempdir().unwrap();
    let future = Local::now().date_naive() + chrono::Duration::days(40);
    let store = Store::new(temp.path().to_path_buf());
    store.add_on_date(future, "first", Status::Done).unwrap();
    store.add_on_date(future, "second", Status::Done).unwrap();
    assert!(store.active().unwrap().is_none());
    success(
        &store.dir,
        &[
            "list",
            &future.to_string(),
            "--repeat",
            "1-2",
            "--date",
            "2026-09-30",
        ],
    );
    assert_eq!(
        titles(&store, "2026-09-30", Status::Todo),
        ["first", "second"]
    );
    assert_eq!(
        titles(&store, &future.to_string(), Status::Done),
        ["first", "second"]
    );
}
