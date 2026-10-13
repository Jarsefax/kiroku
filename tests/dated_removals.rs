use kiroku::storage::{Status, Store, parse_date};
use std::fs;

fn date(text: &str) -> chrono::NaiveDate {
    parse_date(text).unwrap()
}

#[test]
fn dated_removal_preserves_duplicate_occurrences_comments_and_other_months() {
    let temp = tempfile::tempdir().unwrap();
    let store = Store::new(temp.path().to_path_buf());
    let source_path = store.dir.join("2026-09.md");
    let source = "# 2026-09-30\r\n\r\n## To do\r\n- duplicate 日本語\r\n\r\n## Doing\r\n- duplicate 日本語\r\n<!-- keep this comment -->\r\n\r\n## Done\r\n- keep";
    fs::write(&source_path, source).unwrap();
    store
        .add_on_date(date("2026-10-02"), "active", Status::Todo)
        .unwrap();
    store
        .add_on_date(date("2026-10-02"), "second", Status::Done)
        .unwrap();
    let active_path = store.dir.join("2026-10.md");
    let active_bytes = fs::read(&active_path).unwrap();
    let active_backup = store.dir.join("backups/2026-10.md.000001.bak");
    let backup_bytes = fs::read(&active_backup).unwrap();
    let source_day = store
        .days(Some((date("2026-09-30"), date("2026-09-30"))))
        .unwrap()
        .pop()
        .unwrap();
    let day = store
        .remove_on_date(source_day.date, 2, Some(&source_day))
        .unwrap();
    assert_eq!(day.items.len(), 2);
    assert_eq!(day.items[0].title, "duplicate 日本語");
    assert_eq!(day.items[0].status, Status::Todo);
    assert_eq!(day.items[1].title, "keep");
    assert_eq!(
        fs::read_to_string(&source_path).unwrap(),
        format!(
            "<!-- kiroku-format: 1 -->\r\n\r\n{}",
            source.replace("## Doing\r\n- duplicate 日本語\r\n", "## Doing\r\n")
        )
    );
    assert_eq!(
        fs::read_to_string(store.dir.join("backups/2026-09.md.000001.bak")).unwrap(),
        source
    );
    assert_eq!(fs::read(&active_path).unwrap(), active_bytes);
    assert_eq!(fs::read(&active_backup).unwrap(), backup_bytes);
    assert_eq!(store.active().unwrap().unwrap().date, date("2026-10-02"));
}

#[test]
fn dated_removal_rejects_stale_snapshots_and_invalid_sources_without_saving() {
    let temp = tempfile::tempdir().unwrap();
    let store = Store::new(temp.path().to_path_buf());
    let stale = store
        .add_on_date(date("2026-10-02"), "original", Status::Todo)
        .unwrap();
    let current = store
        .add_on_date(stale.date, "new item", Status::Todo)
        .unwrap();
    let path = store.dir.join("2026-10.md");
    let bytes = fs::read(&path).unwrap();
    let backups = fs::read_dir(store.dir.join("backups")).unwrap().count();
    let error = store
        .remove_on_date(stale.date, 1, Some(&stale))
        .unwrap_err()
        .to_string();
    assert!(error.contains("source day changed"));
    let mut wrong_date = current.clone();
    wrong_date.date = date("2026-10-03");
    for (target, number, expected) in [
        (stale.date, 0, Some(&current)),
        (stale.date, 3, Some(&current)),
        (stale.date, usize::MAX, Some(&current)),
        (stale.date, 1, Some(&wrong_date)),
        (date("2026-10-03"), 1, None),
        (date("2026-09-30"), 1, None),
    ] {
        assert!(store.remove_on_date(target, number, expected).is_err());
        assert_eq!(fs::read(&path).unwrap(), bytes);
        assert_eq!(
            fs::read_dir(store.dir.join("backups")).unwrap().count(),
            backups
        );
        assert!(!store.dir.join("2026-09.md").exists());
    }
}
