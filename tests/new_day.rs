use kiroku::{
    config,
    storage::{Change, Status, Store, parse_date},
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
        assert_eq!(
            days[0]
                .items
                .iter()
                .map(|item| (item.title.as_str(), item.status))
                .collect::<Vec<_>>(),
            [("carry", Status::Doing), ("foo", Status::Done)]
        );
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
    assert_eq!(
        days[0]
            .items
            .iter()
            .map(|item| item.status)
            .collect::<Vec<_>>(),
        [Status::Doing, Status::Done]
    );
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
fn same_month_rollover_moves_todos_copies_doing_and_preserves_comments_and_history_bytes() {
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
        assert_eq!(
            days[1]
                .items
                .iter()
                .map(|item| item.status)
                .collect::<Vec<_>>(),
            [Status::Doing, Status::Done]
        );
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
            .replace(&format!("## To do{newline}"), "")
            .replacen(&format!("- duplicate{newline}"), "", 1)
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
fn cross_month_rollover_carries_only_the_latest_eligible_day_and_preserves_future_records() {
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
    assert_eq!(
        days[0]
            .items
            .iter()
            .map(|item| (item.title.as_str(), item.status))
            .collect::<Vec<_>>(),
        [("duplicate", Status::Doing), ("finished", Status::Done)]
    );
    assert_eq!(days[1].items.len(), 2);
    assert!(
        days[1]
            .items
            .iter()
            .all(|item| item.status == Status::Todo && item.title == "duplicate")
    );
    assert_eq!(days[2].items.len(), 3);
    let saved_source = fs::read_to_string(&source_path).unwrap();
    assert_eq!(
        saved_source,
        format!(
            "<!-- kiroku-format: 2 -->\r\n\r\n{}",
            source.replacen("## To do\r\n- duplicate\r\n", "", 1)
        )
    );
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
fn copying_doing_without_todos_preserves_history_and_notes_and_removes_the_empty_heading() {
    for previous in ["2026-10-01", "2026-09-30"] {
        for newline in ["\n", "\r\n"] {
            let temp = tempfile::tempdir().unwrap();
            let store = Store::new(temp.path().to_path_buf());
            let source_path = store
                .dir
                .join(format!("{}.md", date(previous).format("%Y-%m")));
            // Closing a legacy day upgrades its file while retaining notes.
            let source = format!("# {previous}\n## To do\n<!-- pending note -->\n## Doing\n- duplicate  \n<!-- doing note -->\n- duplicate\n## Done\n- finished\n").replace('\n', newline);
            fs::write(&source_path, &source).unwrap();

            assert!(store.new_day(date("2026-10-02")).unwrap());
            let days = store.days(None).unwrap();
            assert_eq!(
                days[0]
                    .items
                    .iter()
                    .map(|item| (item.title.as_str(), item.status))
                    .collect::<Vec<_>>(),
                [
                    ("duplicate", Status::Doing),
                    ("duplicate", Status::Doing),
                    ("finished", Status::Done)
                ]
            );
            assert_eq!(
                days[1]
                    .items
                    .iter()
                    .map(|item| (item.title.as_str(), item.status))
                    .collect::<Vec<_>>(),
                [("duplicate", Status::Todo), ("duplicate", Status::Todo)]
            );
            let saved_source = fs::read_to_string(&source_path).unwrap();
            let history = source.replace(&format!("## To do{newline}"), "");
            assert!(saved_source.starts_with(&format!(
                "<!-- kiroku-format: 2 -->{newline}{newline}{history}"
            )));
            if previous == "2026-09-30" {
                assert_eq!(
                    saved_source,
                    format!("<!-- kiroku-format: 2 -->{newline}{newline}{history}")
                );
                assert!(!store.dir.join("backups/2026-09.md.000001.bak").exists());
            } else {
                assert_eq!(
                    fs::read_to_string(store.dir.join("backups/2026-10.md.000001.bak")).unwrap(),
                    source
                );
            }
            assert!(!store.new_day(date("2026-10-02")).unwrap());
            assert_eq!(fs::read_to_string(source_path).unwrap(), saved_source);
        }
    }
}

#[test]
fn doing_history_survives_later_completion_without_recopying_older_days() {
    let temp = tempfile::tempdir().unwrap();
    let store = Store::new(temp.path().to_path_buf());
    store
        .add_on_date(date("2026-09-30"), "pending", Status::Todo)
        .unwrap();
    store
        .add_on_date(date("2026-09-30"), "ongoing", Status::Doing)
        .unwrap();
    store
        .add_on_date(date("2026-09-30"), "finished", Status::Done)
        .unwrap();

    store.new_day(date("2026-10-01")).unwrap();
    store.change(Change::Move(2, Status::Doing), None).unwrap();
    store.new_day(date("2026-10-02")).unwrap();
    store.change(Change::Move(2, Status::Done), None).unwrap();
    store.new_day(date("2026-10-05")).unwrap();

    let days = store.days(None).unwrap();
    assert_eq!(days.len(), 4);
    for day in &days[..2] {
        assert!(
            day.items
                .iter()
                .any(|item| item.title == "ongoing" && item.status == Status::Doing)
        );
        assert!(!day.items.iter().any(|item| item.status == Status::Todo));
    }
    assert_eq!(
        days[2]
            .items
            .iter()
            .map(|item| (item.title.as_str(), item.status))
            .collect::<Vec<_>>(),
        [("ongoing", Status::Done)]
    );
    assert_eq!(
        days[3]
            .items
            .iter()
            .map(|item| (item.title.as_str(), item.status))
            .collect::<Vec<_>>(),
        [("pending", Status::Todo)]
    );
}

#[test]
fn backup_failure_leaves_source_items_and_missing_today_unchanged() {
    for (source_date, today) in [("2026-10-01", "2026-10-02"), ("2026-09-30", "2026-10-02")] {
        let temp = tempfile::tempdir().unwrap();
        let store = configured_store(&temp, WEEKLY_TODOS);
        let path = store
            .dir
            .join(format!("{}.md", date(source_date).format("%Y-%m")));
        fs::write(
            &path,
            format!("# {source_date}\n## To do\n- pending\n## Doing\n- unfinished\n## Done\n"),
        )
        .unwrap();
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
fn concurrent_new_day_calls_carry_items_once() {
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
    assert_eq!(
        days[0]
            .items
            .iter()
            .map(|item| (item.title.as_str(), item.status))
            .collect::<Vec<_>>(),
        [("doing", Status::Doing), ("done", Status::Done)]
    );
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

#[test]
fn rollover_removes_empty_source_days_without_filling_skipped_dates() {
    for previous in ["2026-10-01", "2026-09-30", "2026-08-31"] {
        for newline in ["\n", "\r\n"] {
            for (header, body, carried) in [
                ("", "## To do\n\n## Doing\n\n## Done\n", 0),
                (
                    "<!-- kiroku-format: 1 -->\n\n",
                    "## To do\n- duplicate\n- duplicate\n## Doing\n## Done\n",
                    2,
                ),
                ("<!-- kiroku-format: 2 -->\n\n", "", 0),
            ] {
                let temp = tempfile::tempdir().unwrap();
                let store = Store::new(temp.path().to_path_buf());
                let path = store
                    .dir
                    .join(format!("{}.md", date(previous).format("%Y-%m")));
                let original = format!("{header}# {previous}\n{body}")
                    .trim_end_matches('\n')
                    .replace('\n', newline);
                fs::write(&path, &original).unwrap();
                assert!(store.new_day(date("2026-10-02")).unwrap());

                let days = store.days(None).unwrap();
                assert_eq!(days.len(), 1);
                assert_eq!(days[0].date, date("2026-10-02"));
                assert_eq!(days[0].items.len(), carried);
                assert!(
                    days[0]
                        .items
                        .iter()
                        .all(|item| item.title == "duplicate" && item.status == Status::Todo)
                );
                assert!(store.previous(date("2026-10-02")).unwrap().is_none());
                assert!(store.file_path(Some(date(previous))).is_err());
                let saved = fs::read_to_string(&path).unwrap();
                assert!(!saved.contains(&format!("# {previous}")));
                if previous == "2026-10-01" {
                    assert_eq!(
                        fs::read_to_string(store.dir.join("backups/2026-10.md.000001.bak"))
                            .unwrap(),
                        original
                    );
                } else {
                    assert_eq!(saved.trim(), "<!-- kiroku-format: 2 -->");
                    if newline == "\r\n" {
                        assert!(!saved.replace("\r\n", "").contains('\n'));
                    }
                }
                if previous == "2026-08-31" {
                    assert!(!store.dir.join("2026-09.md").exists());
                }
                let destination = store.dir.join("2026-10.md");
                let today_bytes = fs::read(&destination).unwrap();
                assert!(!store.new_day(date("2026-10-02")).unwrap());
                assert_eq!(fs::read_to_string(path).unwrap(), saved);
                assert_eq!(fs::read(destination).unwrap(), today_bytes);
            }
        }
    }
}

#[test]
fn gaps_and_deleted_days_allow_rollover_listing_and_later_dated_edits() {
    for previous in ["2026-10-02", "2026-09-30"] {
        let temp = tempfile::tempdir().unwrap();
        let store = Store::new(temp.path().to_path_buf());
        let older_date = date(previous) - chrono::Duration::days(1);
        let older = format!("# {older_date}\n## To do\n## Doing\n## Done\n- earlier history\n\n");
        let source = format!("# {previous}\n## To do\n- carried\n## Doing\n## Done\n");
        let future = "# 2026-10-20\n## To do\n- future plan\n## Doing\n## Done\n";
        let source_path = store
            .dir
            .join(format!("{}.md", date(previous).format("%Y-%m")));
        let destination = store.dir.join("2026-10.md");
        if previous == "2026-10-02" {
            fs::write(&source_path, format!("{older}{source}{future}")).unwrap();
        } else {
            fs::write(&source_path, format!("{older}{source}")).unwrap();
            fs::write(&destination, future).unwrap();
        }

        store.new_day(date("2026-10-05")).unwrap();
        store.new_day(date("2026-10-08")).unwrap();
        let days = store.days(None).unwrap();
        assert_eq!(
            days.iter().map(|day| day.date).collect::<Vec<_>>(),
            [older_date, date("2026-10-08"), date("2026-10-20")]
        );
        assert_eq!(
            store.previous(date("2026-10-08")).unwrap().unwrap().date,
            older_date
        );
        let active = store.active_on(date("2026-10-08")).unwrap().unwrap();
        assert_eq!(active.date, date("2026-10-08"));
        assert_eq!(active.items[0].title, "carried");
        let view = kiroku::listing::select(&store, Some("2026-10-06"), date("2026-10-08")).unwrap();
        assert!(view.days.is_empty());
        let view = kiroku::listing::select(
            &store,
            Some(&format!("{older_date}..2026-10-20")),
            date("2026-10-08"),
        )
        .unwrap();
        assert_eq!(view.days.len(), 3);
        assert!(fs::read_to_string(&destination).unwrap().ends_with(future));

        store
            .add_on_date(date(previous), "backfilled", Status::Done)
            .unwrap();
        assert_eq!(
            store
                .days(None)
                .unwrap()
                .iter()
                .map(|day| day.date)
                .collect::<Vec<_>>(),
            [
                older_date,
                date(previous),
                date("2026-10-08"),
                date("2026-10-20")
            ]
        );
        assert_eq!(
            store.active_on(date("2026-10-08")).unwrap().unwrap().date,
            active.date
        );
        assert_eq!(store.file_path(Some(date(previous))).unwrap(), source_path);
        let saved = fs::read(&destination).unwrap();
        assert!(!store.new_day(date("2026-10-08")).unwrap());
        assert_eq!(fs::read(destination).unwrap(), saved);
    }
}

#[test]
fn rollover_leaves_older_empty_records_unchanged() {
    let temp = tempfile::tempdir().unwrap();
    let store = Store::new(temp.path().to_path_buf());
    let path = store.dir.join("2026-10.md");
    let older = "# 2026-10-01\n\n";
    fs::write(
        &path,
        format!("<!-- kiroku-format: 2 -->\n\n{older}# 2026-10-02\n## To do\n- pending\n"),
    )
    .unwrap();
    store.new_day(date("2026-10-05")).unwrap();
    assert_eq!(
        store
            .days(None)
            .unwrap()
            .iter()
            .map(|day| day.date)
            .collect::<Vec<_>>(),
        [date("2026-10-01"), date("2026-10-05")]
    );
    assert!(fs::read_to_string(path).unwrap().contains(older));
}
