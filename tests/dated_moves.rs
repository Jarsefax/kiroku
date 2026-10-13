use kiroku::storage::{Status, Store, parse_date};
use std::fs;

fn date(text: &str) -> chrono::NaiveDate {
    parse_date(text).unwrap()
}

fn day(store: &Store, text: &str) -> kiroku::storage::Day {
    store
        .days(Some((date(text), date(text))))
        .unwrap()
        .pop()
        .unwrap()
}

#[test]
fn same_month_moves_insert_dates_chronologically_and_preserve_other_lines() {
    let temp = tempfile::tempdir().unwrap();
    let store = Store::new(temp.path().to_path_buf());
    let path = store.dir.join("2026-10.md");
    let original = "# 2026-10-06\r\n\r\n## To do\r\n- duplicate 日本語\r\n<!-- note stays here -->\r\n- duplicate 日本語\r\n\r\n## Doing\r\n\r\n## Done\r\n- completed";
    fs::write(&path, original).unwrap();
    let destination = store
        .move_on_date_at(
            date("2026-10-03"),
            Some(date("2026-10-06")),
            2,
            Status::Doing,
            None,
            None,
        )
        .unwrap();
    assert_eq!(destination.items.len(), 1);
    assert_eq!(destination.items[0].title, "duplicate 日本語");
    assert_eq!(destination.items[0].status, Status::Doing);
    let source = day(&store, "2026-10-06");
    assert_eq!(source.items.len(), 2);
    assert_eq!(source.items[0].title, "duplicate 日本語");
    assert_eq!(source.items[1].title, "completed");
    let contents = fs::read_to_string(&path).unwrap();
    assert!(contents.contains("- duplicate 日本語\r\n<!-- note stays here -->\r\n"));
    assert!(contents.ends_with("- completed"));
    assert!(!contents.replace("\r\n", "").contains('\n'));
    assert_eq!(
        fs::read_to_string(store.dir.join("backups/2026-10.md.000001.bak")).unwrap(),
        original
    );
    store
        .move_on_date_at(
            date("2026-10-08"),
            Some(date("2026-10-03")),
            1,
            Status::Done,
            None,
            None,
        )
        .unwrap();
    assert!(day(&store, "2026-10-03").items.is_empty());
    assert_eq!(
        store
            .days(None)
            .unwrap()
            .iter()
            .map(|day| day.date)
            .collect::<Vec<_>>(),
        ["2026-10-03", "2026-10-06", "2026-10-08"].map(date)
    );
}

#[test]
fn cross_month_moves_preserve_both_formats_and_backup_both_originals() {
    let temp = tempfile::tempdir().unwrap();
    let store = Store::new(temp.path().to_path_buf());
    let source_path = store.dir.join("2026-09.md");
    let destination_path = store.dir.join("2026-10.md");
    let source = "# 2026-09-30\r\n\r\n## To do\r\n- duplicate\r\n\r\n## Doing\r\n<!-- source comment -->\r\n- duplicate\r\n\r\n## Done\r\n- keep\r\n";
    let destination = "<!-- kiroku-format: 1 -->\n\n# 2026-10-06\n\n## To do\n- destination todo\n\n## Doing\n\n## Done\n<!-- destination comment -->\n- finished\n";
    fs::write(&source_path, source).unwrap();
    fs::write(&destination_path, destination).unwrap();
    let moved = store
        .move_on_date_at(
            date("2026-10-06"),
            Some(date("2026-09-30")),
            2,
            Status::Done,
            None,
            Some(1),
        )
        .unwrap();
    assert_eq!(
        moved
            .numbered()
            .iter()
            .map(|item| item.title.as_str())
            .collect::<Vec<_>>(),
        ["destination todo", "duplicate", "finished"]
    );
    assert_eq!(day(&store, "2026-09-30").items.len(), 2);
    assert!(
        day(&store, "2026-09-30")
            .items
            .iter()
            .all(|item| item.status != Status::Doing)
    );
    let saved_source = fs::read_to_string(source_path).unwrap();
    assert!(saved_source.contains("<!-- source comment -->\r\n"));
    assert!(!saved_source.replace("\r\n", "").contains('\n'));
    let saved_destination = fs::read_to_string(destination_path).unwrap();
    assert!(saved_destination.contains("<!-- destination comment -->\n"));
    assert!(!saved_destination.contains('\r'));
    for (month, original) in [("2026-09", source), ("2026-10", destination)] {
        assert_eq!(
            fs::read_to_string(store.dir.join(format!("backups/{month}.md.000001.bak"))).unwrap(),
            original
        );
    }
}

#[test]
fn same_day_moves_support_positions_and_leave_noops_untouched() {
    let temp = tempfile::tempdir().unwrap();
    let store = Store::new(temp.path().to_path_buf());
    let path = store.dir.join("2026-10.md");
    let original = "# 2026-10-06\n\n## To do\n- first\n- second\n\n## Doing\n\n## Done\n";
    fs::write(&path, original).unwrap();
    store
        .move_on_date_at(date("2026-10-06"), None, 1, Status::Todo, None, None)
        .unwrap();
    assert_eq!(fs::read_to_string(&path).unwrap(), original);
    assert!(!store.dir.join("backups").exists());
    let moved = store
        .move_on_date_at(date("2026-10-06"), None, 1, Status::Todo, None, Some(2))
        .unwrap();
    assert_eq!(
        moved
            .numbered()
            .iter()
            .map(|item| item.title.as_str())
            .collect::<Vec<_>>(),
        ["second", "first"]
    );
    let moved = store
        .move_on_date_at(date("2026-10-06"), None, 2, Status::Done, None, None)
        .unwrap();
    assert_eq!(moved.items.len(), 2);
    assert_eq!(moved.items[1].status, Status::Done);
}

#[test]
fn invalid_sources_positions_and_stale_snapshots_do_not_create_destinations() {
    let temp = tempfile::tempdir().unwrap();
    let store = Store::new(temp.path().to_path_buf());
    store
        .add_on_date(date("2026-10-06"), "original", Status::Todo)
        .unwrap();
    let stale = day(&store, "2026-10-06");
    let mut changed = stale.clone();
    changed.items[0].title = "stale title".into();
    let path = store.dir.join("2026-10.md");
    let original = fs::read(&path).unwrap();
    for (from, number, expected, position) in [
        (Some(date("2026-10-06")), 0, None, None),
        (Some(date("2026-10-06")), 2, None, None),
        (Some(date("2026-10-05")), 1, None, None),
        (None, 1, None, Some(0)),
        (None, 1, None, Some(2)),
        (None, 1, Some(&changed), None),
    ] {
        assert!(
            store
                .move_on_date_at(
                    date("2026-09-30"),
                    from,
                    number,
                    Status::Doing,
                    expected,
                    position
                )
                .is_err()
        );
        assert_eq!(fs::read(&path).unwrap(), original);
        assert!(!store.dir.join("2026-09.md").exists());
        assert!(!store.dir.join("backups").exists());
    }
    for invalid in ["<!-- kiroku-format: 2 -->\n", "unrecognized content\n"] {
        let destination = store.dir.join("2026-09.md");
        fs::write(&destination, invalid).unwrap();
        assert!(
            store
                .move_on_date_at(date("2026-09-30"), None, 1, Status::Todo, None, None)
                .is_err()
        );
        assert_eq!(fs::read(&path).unwrap(), original);
        assert_eq!(fs::read_to_string(destination).unwrap(), invalid);
    }
}

#[test]
fn unavailable_source_backup_does_not_create_a_cross_month_destination() {
    let temp = tempfile::tempdir().unwrap();
    let store = Store::new(temp.path().to_path_buf());
    store
        .add_on_date(date("2026-10-06"), "source", Status::Todo)
        .unwrap();
    let path = store.dir.join("2026-10.md");
    let original = fs::read(&path).unwrap();
    fs::write(store.dir.join("backups"), "blocks backups").unwrap();
    assert!(
        store
            .move_on_date_at(date("2026-09-30"), None, 1, Status::Todo, None, None)
            .is_err()
    );
    assert_eq!(fs::read(path).unwrap(), original);
    assert!(!store.dir.join("2026-09.md").exists());
}

#[test]
fn concurrent_cross_month_moves_serialize_without_losing_or_duplicating_items() {
    let temp = tempfile::tempdir().unwrap();
    let store = Store::new(temp.path().to_path_buf());
    for n in 0..8 {
        store
            .add_on_date(date("2026-09-30"), &format!("item {n}"), Status::Todo)
            .unwrap();
    }
    let threads: Vec<_> = (0..8)
        .map(|_| {
            let store = store.clone();
            std::thread::spawn(move || {
                store
                    .move_on_date_at(
                        date("2026-10-06"),
                        Some(date("2026-09-30")),
                        1,
                        Status::Doing,
                        None,
                        None,
                    )
                    .unwrap()
            })
        })
        .collect();
    for thread in threads {
        thread.join().unwrap();
    }
    assert!(day(&store, "2026-09-30").items.is_empty());
    let destination = day(&store, "2026-10-06");
    let mut titles: Vec<_> = destination
        .items
        .iter()
        .map(|item| item.title.as_str())
        .collect();
    titles.sort();
    assert_eq!(
        titles,
        [
            "item 0", "item 1", "item 2", "item 3", "item 4", "item 5", "item 6", "item 7"
        ]
    );
    assert!(
        destination
            .items
            .iter()
            .all(|item| item.status == Status::Doing)
    );
}
