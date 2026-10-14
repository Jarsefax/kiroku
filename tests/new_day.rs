use kiroku::{
    config,
    storage::{Status, Store, parse_date},
};
use std::fs;

fn date(text: &str) -> chrono::NaiveDate {
    parse_date(text).unwrap()
}

fn configured_store(temp: &tempfile::TempDir, config: &str) -> Store {
    let config_dir = temp.path().join("config");
    fs::create_dir_all(config_dir.join("kiroku")).unwrap();
    fs::write(config_dir.join("kiroku/config.toml"), config).unwrap();
    Store::new(temp.path().to_path_buf())
        .with_recurring_todos(config::recurring_todos_with(&config_dir).unwrap())
        .unwrap()
}

const WEEKLY_TODOS: &str = r#"
[[recurring_todos]]
title = "foo"
days = ["monday", "tuesday", "thursday", "friday"]

[[recurring_todos]]
title = "bar"
days = ["wednesday"]
"#;

#[test]
fn recurring_todos_seed_the_first_day_on_each_matching_weekday() {
    let config = format!(
        r#"{WEEKLY_TODOS}
[[recurring_todos]]
title = "weekend 日本語"
days = ["saturday", "sunday"]
"#
    );
    for (today, expected) in [
        ("2026-10-05", "foo"),
        ("2026-10-06", "foo"),
        ("2026-10-07", "bar"),
        ("2026-10-08", "foo"),
        ("2026-10-09", "foo"),
        ("2026-10-10", "weekend 日本語"),
        ("2026-10-11", "weekend 日本語"),
    ] {
        let temp = tempfile::tempdir().unwrap();
        let store = configured_store(&temp, &config);
        assert!(store.new_day(date(today)).unwrap());
        let days = store.days(None).unwrap();
        assert_eq!(days.len(), 1);
        assert_eq!(days[0].items.len(), 1, "{today}");
        assert_eq!(days[0].items[0].title, expected, "{today}");
        assert_eq!(days[0].items[0].status, Status::Todo);
    }
    let temp = tempfile::tempdir().unwrap();
    let store = configured_store(&temp, WEEKLY_TODOS);
    store.new_day(date("2026-10-10")).unwrap();
    assert!(
        store
            .active_on(date("2026-10-10"))
            .unwrap()
            .unwrap()
            .items
            .is_empty()
    );
}

#[test]
fn recurring_todos_reappear_after_completion_and_add_titles_missing_from_history() {
    for previous in ["2026-10-01", "2026-09-30"] {
        let temp = tempfile::tempdir().unwrap();
        let config = format!(
            r#"{WEEKLY_TODOS}
[[recurring_todos]]
title = "never recorded"
days = ["friday"]
"#
        );
        let store = configured_store(&temp, &config);
        store
            .add_on_date(date(previous), "foo", Status::Done)
            .unwrap();
        store
            .add_on_date(date(previous), "carry", Status::Doing)
            .unwrap();
        store
            .add_on_date(date("2026-10-20"), "future", Status::Todo)
            .unwrap();

        assert!(store.new_day(date("2026-10-02")).unwrap());
        let days = store.days(None).unwrap();
        assert_eq!(
            days.iter().map(|day| day.date).collect::<Vec<_>>(),
            [date(previous), date("2026-10-02"), date("2026-10-20")]
        );
        assert_eq!(days[0].items.len(), 1);
        assert_eq!(days[0].items[0].title, "foo");
        assert_eq!(days[0].items[0].status, Status::Done);
        assert_eq!(
            days[1]
                .items
                .iter()
                .map(|item| item.title.as_str())
                .collect::<Vec<_>>(),
            ["carry", "foo", "never recorded"]
        );
        assert!(days[1].items.iter().all(|item| item.status == Status::Todo));
        assert_eq!(days[2].items.len(), 1);
        assert_eq!(days[2].items[0].title, "future");
        let saved = fs::read(store.dir.join("2026-10.md")).unwrap();
        assert!(!store.new_day(date("2026-10-02")).unwrap());
        assert_eq!(fs::read(store.dir.join("2026-10.md")).unwrap(), saved);
    }
}

#[test]
fn recurring_todos_append_in_config_order_without_duplicating_carried_titles() {
    let temp = tempfile::tempdir().unwrap();
    let store = configured_store(
        &temp,
        r#"
[[recurring_todos]]
title = "foo"
days = ["monday", "monday"]
[[recurring_todos]]
title = "second"
days = ["monday"]
[[recurring_todos]]
title = " foo "
days = ["monday"]
[[recurring_todos]]
title = "second"
days = ["monday"]
[[recurring_todos]]
title = "Foo"
days = ["monday"]
"#,
    );
    for status in [Status::Todo, Status::Todo, Status::Doing, Status::Done] {
        store
            .add_on_date(date("2026-10-02"), "foo", status)
            .unwrap();
    }
    store.new_day(date("2026-10-05")).unwrap();
    let days = store.days(None).unwrap();
    assert_eq!(days[0].items.len(), 1);
    assert_eq!(days[0].items[0].status, Status::Done);
    assert_eq!(
        days[1]
            .items
            .iter()
            .map(|item| item.title.as_str())
            .collect::<Vec<_>>(),
        ["foo", "foo", "foo", "second", "Foo"]
    );
}

#[test]
fn recurrence_only_applies_when_new_day_creates_the_record() {
    let temp = tempfile::tempdir().unwrap();
    let store = configured_store(&temp, WEEKLY_TODOS);
    store
        .add_on_date(date("2026-10-05"), "explicit date", Status::Todo)
        .unwrap();
    let path = store.dir.join("2026-10.md");
    let original = fs::read(&path).unwrap();
    assert!(!store.new_day(date("2026-10-05")).unwrap());
    assert_eq!(fs::read(path).unwrap(), original);
    assert!(!store.dir.join("backups").exists());
}

#[test]
fn same_month_rollover_moves_duplicates_in_order_and_preserves_comments_and_done_bytes() {
    for newline in ["\n", "\r\n"] {
        let temp = tempfile::tempdir().unwrap();
        let store = Store::new(temp.path().to_path_buf());
        let path = store.dir.join("2026-10.md");
        let older =
            "# 2026-10-01\n## To do\n- older unfinished\n## Doing\n## Done\n- older done\n\n"
                .replace('\n', newline);
        let source = "# 2026-10-02\n\n## To do\n- duplicate\n<!-- todo note -->\n- möte 日本語\n\n## Doing\n<!-- doing note\nkeep both lines\n-->\n- duplicate\n\n## Done\n- duplicate  \n<!-- done note -->\n".replace('\n', newline);
        let original = format!("<!-- kiroku-format: 1 -->{newline}{newline}{older}{source}");
        fs::write(&path, &original).unwrap();
        assert!(store.new_day(date("2026-10-05")).unwrap());
        let days = store.days(None).unwrap();
        assert_eq!(
            days.iter().map(|day| day.date).collect::<Vec<_>>(),
            ["2026-10-01", "2026-10-02", "2026-10-05"].map(date)
        );
        assert_eq!(days[0].items.len(), 2);
        assert_eq!(days[1].items.len(), 1);
        assert_eq!(days[1].items[0].status, Status::Done);
        assert_eq!(
            days[2]
                .items
                .iter()
                .map(|item| item.title.as_str())
                .collect::<Vec<_>>(),
            ["duplicate", "möte 日本語", "duplicate"]
        );
        assert!(days[2].items.iter().all(|item| item.status == Status::Todo));
        let saved = fs::read_to_string(&path).unwrap();
        let expected_source = source
            .replace(&format!("- duplicate{newline}"), "")
            .replace(&format!("- möte 日本語{newline}"), "");
        assert!(saved.contains(&older));
        assert!(saved.contains(&expected_source));
        if newline == "\r\n" {
            assert!(!saved.replace("\r\n", "").contains('\n'));
        }
        assert_eq!(
            fs::read_to_string(store.dir.join("backups/2026-10.md.000001.bak")).unwrap(),
            original
        );
        assert!(!store.new_day(date("2026-10-05")).unwrap());
        assert_eq!(fs::read_to_string(&path).unwrap(), saved);
        assert_eq!(fs::read_dir(store.dir.join("backups")).unwrap().count(), 1);
    }
}

#[test]
fn cross_month_rollover_moves_only_the_latest_eligible_day_and_preserves_future_records() {
    let temp = tempfile::tempdir().unwrap();
    let store = Store::new(temp.path().to_path_buf());
    let source_path = store.dir.join("2026-09.md");
    let destination_path = store.dir.join("2026-10.md");
    let source = "# 2026-09-30\r\n## To do\r\n- duplicate\r\n<!-- keep todo note -->\r\n## Doing\r\n- duplicate\r\n## Done\r\n- finished\r\n";
    let future =
        "# 2026-10-20\n## To do\n- future todo\n## Doing\n- future doing\n## Done\n- future done\n";
    fs::write(&source_path, source).unwrap();
    fs::write(&destination_path, future).unwrap();
    assert!(store.new_day(date("2026-10-02")).unwrap());
    let days = store.days(None).unwrap();
    assert_eq!(days.len(), 3);
    assert_eq!(days[0].items.len(), 1);
    assert_eq!(days[0].items[0].title, "finished");
    assert_eq!(days[0].items[0].status, Status::Done);
    assert_eq!(days[1].items.len(), 2);
    assert!(
        days[1]
            .items
            .iter()
            .all(|item| item.status == Status::Todo && item.title == "duplicate")
    );
    assert_eq!(days[2].items.len(), 3);
    let saved_source = fs::read_to_string(&source_path).unwrap();
    assert!(!saved_source.replace("\r\n", "").contains('\n'));
    assert!(saved_source.contains("<!-- keep todo note -->\r\n"));
    assert!(
        fs::read_to_string(&destination_path)
            .unwrap()
            .ends_with(future)
    );
    assert_eq!(
        fs::read_to_string(store.dir.join("backups/2026-10.md.000001.bak")).unwrap(),
        future
    );
    assert!(!store.dir.join("backups/2026-09.md.000001.bak").exists()); // Usual closed-month cleanup.
    let saved_destination = fs::read(&destination_path).unwrap();
    assert!(!store.new_day(date("2026-10-02")).unwrap());
    assert_eq!(fs::read(destination_path).unwrap(), saved_destination);
    assert_eq!(fs::read_to_string(source_path).unwrap(), saved_source);
}

#[test]
fn backup_failure_leaves_source_items_and_missing_today_unchanged() {
    for (source_date, today) in [("2026-10-01", "2026-10-02"), ("2026-09-30", "2026-10-02")] {
        let temp = tempfile::tempdir().unwrap();
        let store = configured_store(&temp, WEEKLY_TODOS);
        store
            .add_on_date(date(source_date), "unfinished", Status::Doing)
            .unwrap();
        let path = store
            .dir
            .join(format!("{}.md", date(source_date).format("%Y-%m")));
        let original = fs::read(&path).unwrap();
        fs::write(store.dir.join("backups"), "blocks backup creation").unwrap();
        assert!(store.new_day(date(today)).is_err());
        assert_eq!(fs::read(path).unwrap(), original);
        assert!(
            store
                .days(Some((date(today), date(today))))
                .unwrap()
                .is_empty()
        );
    }
}

#[test]
fn concurrent_new_day_calls_move_items_once() {
    let temp = tempfile::tempdir().unwrap();
    let store = configured_store(&temp, WEEKLY_TODOS);
    store
        .add_on_date(date("2026-09-30"), "todo", Status::Todo)
        .unwrap();
    store
        .add_on_date(date("2026-09-30"), "doing", Status::Doing)
        .unwrap();
    store
        .add_on_date(date("2026-09-30"), "done", Status::Done)
        .unwrap();
    let threads: Vec<_> = (0..8)
        .map(|_| {
            let store = store.clone();
            std::thread::spawn(move || store.new_day(date("2026-10-02")).unwrap())
        })
        .collect();
    let created = threads
        .into_iter()
        .filter_map(|thread| thread.join().unwrap().then_some(()))
        .count();
    assert_eq!(created, 1);
    let days = store.days(None).unwrap();
    assert_eq!(days[0].items.len(), 1);
    assert_eq!(days[0].items[0].status, Status::Done);
    assert_eq!(days[1].items.len(), 3);
    assert_eq!(
        days[1]
            .items
            .iter()
            .map(|item| item.title.as_str())
            .collect::<Vec<_>>(),
        ["todo", "doing", "foo"]
    );
}

#[test]
fn an_existing_today_leaves_both_days_unchanged() {
    let temp = tempfile::tempdir().unwrap();
    let store = Store::new(temp.path().to_path_buf());
    store
        .add_on_date(date("2026-09-30"), "unfinished", Status::Doing)
        .unwrap();
    store
        .add_on_date(date("2026-10-02"), "existing", Status::Todo)
        .unwrap();
    let source_path = store.dir.join("2026-09.md");
    let destination_path = store.dir.join("2026-10.md");
    let source = fs::read(&source_path).unwrap();
    let destination = fs::read(&destination_path).unwrap();
    assert!(!store.new_day(date("2026-10-02")).unwrap());
    assert_eq!(fs::read(source_path).unwrap(), source);
    assert_eq!(fs::read(destination_path).unwrap(), destination);
    assert!(!store.dir.join("backups").exists());
}
