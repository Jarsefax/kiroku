use kiroku::storage::{Change, Status, Store, parse_date};
use std::fs;

fn date(text: &str) -> chrono::NaiveDate {
    parse_date(text).unwrap()
}

#[test]
fn dates_insert_before_between_and_after_existing_records_with_exact_backups() {
    let temp = tempfile::tempdir().unwrap();
    let store = Store::new(temp.path().to_path_buf());
    let path = store.dir.join("2026-10.md");
    let first = "# 2026-10-02\r\n\r\n## To do\r\n- first\r\n\r\n## Doing\r\n\r\n## Done\r\n\r\n<!-- keep notes -->\r\n";
    let last = "# 2026-10-06\r\n\r\n## To do\r\n\r\n## Doing\r\n\r\n## Done\r\n- finished";
    fs::write(
        &path,
        format!("<!-- kiroku-format: 1 -->\r\n\r\n{first}{last}"),
    )
    .unwrap();
    for (index, (target, status)) in [
        ("2026-10-04", Status::Doing),
        ("2026-10-01", Status::Done),
        ("2026-10-07", Status::Todo),
        ("2026-10-06", Status::Doing),
    ]
    .into_iter()
    .enumerate()
    {
        let previous = fs::read(&path).unwrap();
        let day = store
            .add_on_date(date(target), "duplicate 日本語", status)
            .unwrap();
        assert_eq!(day.date, date(target));
        assert!(
            day.items
                .iter()
                .any(|item| item.title == "duplicate 日本語" && item.status == status)
        );
        assert_eq!(
            fs::read(
                store
                    .dir
                    .join(format!("backups/2026-10.md.{:06}.bak", index + 1))
            )
            .unwrap(),
            previous
        );
    }
    let days = store.days(None).unwrap();
    assert_eq!(
        days.iter().map(|day| day.date).collect::<Vec<_>>(),
        [
            "2026-10-01",
            "2026-10-02",
            "2026-10-04",
            "2026-10-06",
            "2026-10-07"
        ]
        .map(date)
    );
    assert_eq!(days[1].items.len(), 1);
    assert_eq!(days[1].items[0].title, "first");
    assert_eq!(days[2].items.len(), 1); // No carryover into a new dated record.
    let text = fs::read_to_string(path).unwrap();
    assert!(text.contains(first));
    assert!(text.contains("## Done\r\n- finished\r\n"));
    assert!(!text.replace("\r\n", "").contains('\n'));
    assert_eq!(text.matches("<!-- kiroku-format:").count(), 1);
}

#[test]
fn adding_to_an_older_month_keeps_the_default_destination_and_rollover() {
    let temp = tempfile::tempdir().unwrap();
    let store = Store::new(temp.path().to_path_buf());
    store.new_day(date("2026-10-06")).unwrap();
    store
        .change(Change::Add("unfinished".into(), Status::Doing), None)
        .unwrap();
    let active_path = store.dir.join("2026-10.md");
    let original = fs::read(&active_path).unwrap();
    store
        .add_on_date(date("2026-09-30"), "backfilled", Status::Done)
        .unwrap();
    assert_eq!(fs::read(&active_path).unwrap(), original);
    assert_eq!(store.active().unwrap().unwrap().date, date("2026-10-06"));
    store
        .change(
            Change::Add("default destination".into(), Status::Todo),
            None,
        )
        .unwrap();
    let active = store.active().unwrap().unwrap();
    assert_eq!(active.items.len(), 2);
    assert!(active.items.iter().all(|item| item.title != "backfilled"));
    store.new_day(date("2026-10-07")).unwrap();
    assert_eq!(store.active().unwrap().unwrap().items.len(), 2);
    assert!(
        store
            .active()
            .unwrap()
            .unwrap()
            .items
            .iter()
            .all(|item| item.status == Status::Todo)
    );
    store
        .add_on_date(date("2026-10-08"), "later", Status::Done)
        .unwrap();
    assert_eq!(store.active().unwrap().unwrap().date, date("2026-10-08"));
    assert_eq!(store.active().unwrap().unwrap().items.len(), 1);
}

#[test]
fn dated_copies_keep_sources_and_reject_invalid_or_stale_selections() {
    let temp = tempfile::tempdir().unwrap();
    let store = Store::new(temp.path().to_path_buf());
    store
        .add_on_date(date("2026-10-06"), "123", Status::Done)
        .unwrap();
    let source = store.active().unwrap().unwrap();
    let path = store.dir.join("2026-10.md");
    let original = fs::read(&path).unwrap();
    let day = store
        .repeat_on_date(date("2026-09-30"), None, 1, Status::Doing, Some(&source))
        .unwrap();
    assert_eq!(day.items.len(), 1);
    assert_eq!(day.items[0].title, "123");
    assert_eq!(day.items[0].status, Status::Doing);
    assert_eq!(fs::read(&path).unwrap(), original);
    // Same-day copying remains valid and preserves the source occurrence.
    store
        .repeat_on_date(
            date("2026-10-06"),
            Some(source.date),
            1,
            Status::Todo,
            Some(&source),
        )
        .unwrap();
    assert_eq!(store.active().unwrap().unwrap().items.len(), 2);
    let original = fs::read(&path).unwrap();
    let error = store
        .repeat_on_date(
            date("2026-11-02"),
            Some(source.date),
            1,
            Status::Todo,
            Some(&source),
        )
        .unwrap_err();
    assert!(error.to_string().contains("source day changed"));
    for (from, number) in [(Some(date("2026-10-05")), 1), (None, 0), (None, 99)] {
        assert!(
            store
                .repeat_on_date(date("2026-11-02"), from, number, Status::Todo, None)
                .is_err()
        );
    }
    assert!(!store.dir.join("2026-11.md").exists());
    assert_eq!(fs::read(path).unwrap(), original);
}

#[test]
fn dated_additions_reject_unsupported_malformed_and_nonregular_targets() {
    let temp = tempfile::tempdir().unwrap();
    let store = Store::new(temp.path().to_path_buf());
    let path = store.dir.join("2026-10.md");
    for content in ["<!-- kiroku-format: 2 -->\n", "# 2026-10-06\n## To do\n"] {
        fs::write(&path, content).unwrap();
        assert!(
            store
                .add_on_date(date("2026-10-05"), "new", Status::Todo)
                .is_err()
        );
        assert_eq!(fs::read_to_string(&path).unwrap(), content);
        assert!(!store.dir.join("backups").exists());
    }
    fs::create_dir(store.dir.join("2026-11.md")).unwrap();
    let error = store
        .add_on_date(date("2026-11-02"), "new", Status::Todo)
        .unwrap_err();
    assert!(error.to_string().contains("not a regular log file"));
    #[cfg(unix)]
    {
        let link = store.dir.join("2026-12.md");
        std::os::unix::fs::symlink(&path, &link).unwrap();
        assert!(
            store
                .add_on_date(date("2026-12-02"), "new", Status::Todo)
                .is_err()
        );
        assert_eq!(
            fs::read_to_string(path).unwrap(),
            "# 2026-10-06\n## To do\n"
        );
    }
}

#[test]
fn invalid_titles_do_not_create_dated_records_or_directories() {
    let temp = tempfile::tempdir().unwrap();
    let store = Store::new(temp.path().join("logs"));
    for title in ["", " ", "two\nlines"] {
        assert!(
            store
                .add_on_date(date("2026-10-06"), title, Status::Todo)
                .is_err()
        );
        assert!(!store.dir.exists());
    }
}

#[test]
fn failure_to_back_up_a_target_prevents_creating_a_day_in_that_file() {
    let temp = tempfile::tempdir().unwrap();
    let store = Store::new(temp.path().to_path_buf());
    store
        .add_on_date(date("2026-10-06"), "original", Status::Todo)
        .unwrap();
    let path = store.dir.join("2026-10.md");
    let original = fs::read(&path).unwrap();
    fs::write(store.dir.join("backups"), "blocked").unwrap();
    let error = store
        .add_on_date(date("2026-10-05"), "new", Status::Todo)
        .unwrap_err();
    assert!(format!("{error:#}").contains("Save aborted before replacement"));
    assert_eq!(fs::read(path).unwrap(), original);
    assert_eq!(store.days(None).unwrap().len(), 1);
}

#[test]
fn future_records_are_excluded_from_active_selection_until_their_dates_arrive() {
    let temp = tempfile::tempdir().unwrap();
    let store = Store::new(temp.path().to_path_buf());
    for target in ["2026-09-30", "2026-10-05", "2026-10-20", "2026-11-02"] {
        store
            .add_on_date(date(target), target, Status::Todo)
            .unwrap();
    }
    assert!(store.active_on(date("2026-09-29")).unwrap().is_none());
    for (today, expected) in [
        ("2026-09-30", "2026-09-30"),
        ("2026-10-01", "2026-09-30"),
        ("2026-10-05", "2026-10-05"),
        ("2026-10-19", "2026-10-05"),
        ("2026-10-20", "2026-10-20"),
        ("2026-11-01", "2026-10-20"),
        ("2026-11-02", "2026-11-02"),
    ] {
        assert_eq!(
            store.active_on(date(today)).unwrap().unwrap().date,
            date(expected)
        );
    }
}

#[test]
fn default_mutations_and_copies_ignore_future_records_in_the_same_month() {
    let temp = tempfile::tempdir().unwrap();
    let store = Store::new(temp.path().to_path_buf());
    let today = chrono::Local::now().date_naive();
    // Tomorrow shares the same monthly document on most dates; also exercise
    // the cross-month boundary when the test happens to run on a month end.
    let future = today + chrono::Duration::days(1);
    store.add_on_date(today, "original", Status::Todo).unwrap();
    store
        .add_on_date(future, "future plan", Status::Todo)
        .unwrap();
    store
        .change(Change::Edit(1, "renamed".into()), None)
        .unwrap();
    store.change(Change::Move(1, Status::Done), None).unwrap();
    store.repeat(None, 1, Status::Todo, None, None).unwrap();
    store.change(Change::Remove(2), None).unwrap();
    store
        .change(Change::Add("default addition".into(), Status::Doing), None)
        .unwrap();
    let day = store.active().unwrap().unwrap();
    assert_eq!(day.date, today);
    assert_eq!(day.items.len(), 2);
    let future_day = store.days(Some((future, future))).unwrap().remove(0);
    assert_eq!(future_day.items.len(), 1);
    assert_eq!(future_day.items[0].title, "future plan");
    assert_eq!(future_day.items[0].status, Status::Todo);
}

#[test]
fn starting_today_with_future_records_carries_only_earlier_work_and_is_idempotent() {
    let temp = tempfile::tempdir().unwrap();
    let store = Store::new(temp.path().to_path_buf());
    store
        .add_on_date(date("2026-10-05"), "carry todo", Status::Todo)
        .unwrap();
    store
        .add_on_date(date("2026-10-05"), "carry doing", Status::Doing)
        .unwrap();
    store
        .add_on_date(date("2026-10-05"), "completed", Status::Done)
        .unwrap();
    store
        .add_on_date(date("2026-10-20"), "future plan", Status::Todo)
        .unwrap();
    assert!(store.new_day(date("2026-10-06")).unwrap());
    let days = store.days(None).unwrap();
    assert_eq!(
        days.iter().map(|day| day.date).collect::<Vec<_>>(),
        ["2026-10-05", "2026-10-06", "2026-10-20"].map(date)
    );
    assert_eq!(
        days[1]
            .items
            .iter()
            .map(|item| item.title.as_str())
            .collect::<Vec<_>>(),
        ["carry todo", "carry doing"]
    );
    assert!(days[1].items.iter().all(|item| item.status == Status::Todo));
    assert_eq!(days[2].items.len(), 1);
    let path = store.dir.join("2026-10.md");
    let original = fs::read(&path).unwrap();
    assert!(!store.new_day(date("2026-10-06")).unwrap());
    assert_eq!(fs::read(path).unwrap(), original);
}

#[test]
fn future_only_logs_require_starting_today_before_default_additions() {
    let temp = tempfile::tempdir().unwrap();
    let store = Store::new(temp.path().to_path_buf());
    let today = chrono::Local::now().date_naive();
    store
        .add_on_date(
            today + chrono::Duration::days(1),
            "future plan",
            Status::Todo,
        )
        .unwrap();
    assert!(store.active().unwrap().is_none());
    assert!(
        store
            .change(Change::Add("default".into(), Status::Todo), None)
            .unwrap_err()
            .to_string()
            .contains("new-day")
    );
    assert!(store.repeat(None, 1, Status::Todo, None, None).is_err());
    assert!(store.new_day(today).unwrap());
    assert_eq!(store.active().unwrap().unwrap().date, today);
    assert!(store.active().unwrap().unwrap().items.is_empty());
    assert_eq!(store.days(None).unwrap().len(), 2);
}

#[test]
fn planning_a_future_month_preserves_backups_of_the_active_month() {
    let temp = tempfile::tempdir().unwrap();
    let store = Store::new(temp.path().to_path_buf());
    let today = chrono::Local::now().date_naive();
    store.new_day(today).unwrap();
    let active_path = store.dir.join(format!("{}.md", today.format("%Y-%m")));
    let original = fs::read(&active_path).unwrap();
    store
        .change(Change::Add("active work".into(), Status::Todo), None)
        .unwrap();
    let backup = store
        .dir
        .join(format!("backups/{}.md.000001.bak", today.format("%Y-%m")));
    assert_eq!(fs::read(&backup).unwrap(), original);
    let future = today + chrono::Duration::days(40);
    store
        .add_on_date(future, "future plan", Status::Todo)
        .unwrap();
    store
        .add_on_date(future, "another future plan", Status::Todo)
        .unwrap();
    assert_eq!(fs::read(&backup).unwrap(), original);
    assert_eq!(store.active().unwrap().unwrap().date, today);
    store
        .change(Change::Add("default work".into(), Status::Todo), None)
        .unwrap();
    assert_eq!(fs::read(backup).unwrap(), original);
    assert_eq!(store.active().unwrap().unwrap().items.len(), 2);
}

#[test]
fn concurrent_dated_additions_create_one_record_without_losing_items() {
    let temp = tempfile::tempdir().unwrap();
    let store = Store::new(temp.path().to_path_buf());
    let threads: Vec<_> = (0..8)
        .map(|index| {
            let store = store.clone();
            std::thread::spawn(move || {
                store
                    .add_on_date(date("2026-10-06"), &format!("item {index}"), Status::Todo)
                    .unwrap()
            })
        })
        .collect();
    for thread in threads {
        thread.join().unwrap();
    }
    let days = store.days(None).unwrap();
    assert_eq!(days.len(), 1);
    assert_eq!(days[0].items.len(), 8);
    assert_eq!(fs::read_dir(store.dir.join("backups")).unwrap().count(), 5);
}
