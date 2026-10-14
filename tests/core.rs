use chrono::{Datelike, Duration, NaiveDate};
use kiroku::{
    config, listing,
    storage::{Change, Direction, Status, Store, parse_date},
};
use std::fs;

fn date(text: &str) -> NaiveDate {
    parse_date(text).unwrap()
}
fn fixture() -> (tempfile::TempDir, Store) {
    let temp = tempfile::tempdir().unwrap();
    let store = Store::new(temp.path().join("logs"));
    (temp, store)
}

#[test]
fn storage_changes_and_same_month_rollover_back_up_previous_bytes() {
    let (_temp, store) = fixture();
    store.new_day(date("2026-10-05")).unwrap();
    let path = store.dir.join("2026-10.md");
    assert!(!store.dir.join("backups").exists());
    let first = fs::read(&path).unwrap();
    store
        .change(Change::Add("meeting".into(), Status::Todo), None)
        .unwrap();
    assert_eq!(
        fs::read(store.dir.join("backups/2026-10.md.000001.bak")).unwrap(),
        first
    );
    let previous = fs::read(&path).unwrap();
    store.new_day(date("2026-10-06")).unwrap();
    assert_eq!(
        fs::read(store.dir.join("backups/2026-10.md.000002.bak")).unwrap(),
        previous
    );
    let count = fs::read_dir(store.dir.join("backups")).unwrap().count();
    listing::select(&store, Some("month"), date("2026-10-06")).unwrap();
    assert!(!store.new_day(date("2026-10-06")).unwrap());
    store.change(Change::Move(1, Status::Todo), None).unwrap();
    assert_eq!(
        fs::read_dir(store.dir.join("backups")).unwrap().count(),
        count
    );
}
fn titles(store: &Store) -> Vec<(String, Status)> {
    store
        .active()
        .unwrap()
        .unwrap()
        .numbered()
        .into_iter()
        .map(|i| (i.title.clone(), i.status))
        .collect()
}

#[test]
fn repeat_active_items_preserves_each_source_and_its_exact_title() {
    for source_status in Status::ALL {
        let (_temp, store) = fixture();
        store.new_day(date("2026-10-02")).unwrap();
        store
            .change(Change::Add("möte 日本語".into(), source_status), None)
            .unwrap();
        store
            .change(Change::Add("123".into(), source_status), None)
            .unwrap();
        let source = store.active().unwrap().unwrap();
        store
            .repeat(None, 1, Status::Todo, Some(&source), Some(&source))
            .unwrap();
        let after_copy = store.active().unwrap().unwrap();
        let numeric_title = after_copy
            .numbered()
            .iter()
            .position(|i| i.title == "123")
            .unwrap()
            + 1;
        let day = store
            .repeat(None, numeric_title, Status::Doing, None, None)
            .unwrap();
        assert_eq!(day.items.len(), 4);
        let numbered = day.numbered();
        assert_eq!(numbered[0].title, "möte 日本語");
        assert_eq!(numbered[0].status, Status::Todo);
        assert!(
            day.items
                .iter()
                .any(|i| i.title == "123" && i.status == source_status)
        );
        assert!(
            day.items
                .iter()
                .any(|i| i.title == "möte 日本語" && i.status == source_status)
        );
        assert!(
            day.items
                .iter()
                .any(|i| i.title == "123" && i.status == Status::Doing)
        );
        assert_eq!(day.date, source.date); // Repeating never starts today.
    }
}

#[test]
fn repeat_history_preserves_source_bytes_within_and_across_months() {
    for source_date in ["2026-10-01", "2026-09-30"] {
        let (_temp, store) = fixture();
        fs::create_dir_all(&store.dir).unwrap();
        let source_date = date(source_date);
        let path = store
            .dir
            .join(format!("{}.md", source_date.format("%Y-%m")));
        let original = format!(
            "<!-- keep comments -->\r\n# {source_date}\r\n\r\n## To do\r\n\r\n## Doing\r\n\r\n## Done\r\n- möte 日本語\r\n- möte 日本語\r\n"
        );
        fs::write(&path, &original).unwrap();
        store.new_day(date("2026-10-02")).unwrap();
        store
            .repeat(Some(source_date), 2, Status::Doing, None, None)
            .unwrap();
        assert_eq!(titles(&store), [("möte 日本語".into(), Status::Doing)]);
        let text = fs::read_to_string(&path).unwrap();
        if source_date.month() == 9 {
            assert_eq!(text, original);
        } else {
            assert!(text.starts_with(&format!("<!-- kiroku-format: 1 -->\r\n\r\n{original}")));
            assert!(!text.replace("\r\n", "").contains('\n'));
        }
        assert!(
            store
                .previous(date("2026-10-02"))
                .unwrap()
                .unwrap()
                .items
                .iter()
                .all(|i| i.status == Status::Done)
        );
    }
}

#[test]
fn repeat_rejects_stale_source_and_destination_snapshots() {
    let (_temp, store) = fixture();
    store.new_day(date("2026-09-30")).unwrap();
    store
        .change(Change::Add("original".into(), Status::Done), None)
        .unwrap();
    let source = store.active().unwrap().unwrap();
    store.new_day(date("2026-10-02")).unwrap();
    let active = store.active().unwrap().unwrap();
    let source_path = store.dir.join("2026-09.md");
    let text = fs::read_to_string(&source_path)
        .unwrap()
        .replace("original", "changed externally");
    fs::write(&source_path, text).unwrap();
    let path = store.dir.join("2026-10.md");
    let before = fs::read(&path).unwrap();
    assert!(
        store
            .repeat(
                Some(source.date),
                1,
                Status::Todo,
                Some(&source),
                Some(&active)
            )
            .unwrap_err()
            .to_string()
            .contains("source day changed")
    );
    assert_eq!(fs::read(&path).unwrap(), before);
    let fresh_source = store.previous(active.date).unwrap().unwrap();
    store.new_day(date("2026-10-05")).unwrap();
    let before = fs::read(&path).unwrap();
    assert!(
        store
            .repeat(
                Some(source.date),
                1,
                Status::Todo,
                Some(&fresh_source),
                Some(&active)
            )
            .unwrap_err()
            .to_string()
            .contains("active day changed")
    );
    assert_eq!(fs::read(&path).unwrap(), before);
    let active = store.active().unwrap().unwrap();
    store
        .change(Change::Add("external addition".into(), Status::Todo), None)
        .unwrap();
    let before = fs::read(&path).unwrap();
    assert!(
        store
            .repeat(None, 1, Status::Todo, Some(&active), Some(&active))
            .is_err()
    );
    assert_eq!(fs::read(&path).unwrap(), before);
}

#[test]
fn historical_labels_round_trip_and_invalid_labels_fail() {
    for index in [0, 25, 26, 51, 52, 701, 702, usize::MAX - 1] {
        assert_eq!(
            listing::letter_number(&listing::letters(index)).unwrap(),
            index + 1
        );
    }
    for label in [
        "",
        "a",
        "1",
        "A1",
        " A",
        "日本",
        "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
    ] {
        assert!(listing::letter_number(label).is_err(), "{label:?}");
    }
}

#[test]
fn concurrent_historical_repeats_are_serialized() {
    let (_temp, store) = fixture();
    store.new_day(date("2026-10-01")).unwrap();
    store
        .change(Change::Add("meeting".into(), Status::Done), None)
        .unwrap();
    store.new_day(date("2026-10-02")).unwrap();
    let threads: Vec<_> = (0..8)
        .map(|_| {
            let store = store.clone();
            std::thread::spawn(move || {
                store
                    .repeat(Some(date("2026-10-01")), 1, Status::Todo, None, None)
                    .unwrap()
            })
        })
        .collect();
    for thread in threads {
        thread.join().unwrap();
    }
    assert_eq!(titles(&store).len(), 8);
    assert!(
        titles(&store)
            .iter()
            .all(|i| i == &("meeting".into(), Status::Todo))
    );
    assert_eq!(
        store
            .previous(date("2026-10-02"))
            .unwrap()
            .unwrap()
            .items
            .len(),
        1
    );
}

#[test]
fn reading_empty_store_does_not_create_anything() {
    let (_temp, store) = fixture();
    assert!(store.active().unwrap().is_none());
    let view = listing::select(&store, None, date("2026-10-05")).unwrap();
    assert!(view.days.is_empty());
    assert!(view.notice.contains("No recorded days"));
    assert!(!store.dir.exists());
}

#[test]
fn first_addition_starts_today_in_any_status() {
    for status in Status::ALL {
        let (_temp, store) = fixture();
        let day = store
            .change(Change::Add("  meeting 日本語  ".into(), status), None)
            .unwrap();
        assert_eq!(day.date, chrono::Local::now().date_naive());
        assert_eq!(day.items.len(), 1);
        assert_eq!(day.items[0].title, "meeting 日本語");
        assert_eq!(day.items[0].status, status);
        assert_eq!(store.days(None).unwrap().len(), 1);
        assert!(!store.dir.join("backups").exists());
        assert!(!store.new_day(day.date).unwrap());
    }
}

#[test]
fn invalid_first_additions_and_numbered_changes_do_not_create_a_day() {
    for change in [
        Change::Add("  ".into(), Status::Todo),
        Change::Add("bad\ntitle".into(), Status::Doing),
        Change::Move(1, Status::Done),
        Change::Edit(1, "renamed".into()),
        Change::Remove(1),
    ] {
        let (_temp, store) = fixture();
        assert!(store.change(change, None).is_err());
        assert!(store.active().unwrap().is_none());
        assert!(store.days(None).unwrap().is_empty());
        assert!(!store.dir.join("backups").exists());
    }
}

#[test]
fn filtered_results_keep_source_references_and_combine_text_status_and_dates() {
    let (_temp, store) = fixture();
    store.new_day(date("2026-09-30")).unwrap();
    store
        .change(Change::Add("unrelated".into(), Status::Todo), None)
        .unwrap();
    store
        .change(Change::Add("another thing".into(), Status::Doing), None)
        .unwrap();
    store
        .change(Change::Add("Weekly meeting".into(), Status::Done), None)
        .unwrap();
    store
        .change(Change::Add("weekly meeting".into(), Status::Done), None)
        .unwrap();
    store.new_day(date("2026-10-01")).unwrap();
    store
        .change(Change::Add("WEEKLY meeting".into(), Status::Done), None)
        .unwrap();
    store.new_day(date("2026-10-02")).unwrap();
    store
        .change(
            Change::Add("weekly meeting draft".into(), Status::Doing),
            None,
        )
        .unwrap();
    store
        .change(Change::Add("weekly meeting".into(), Status::Done), None)
        .unwrap();
    let original = fs::read(store.dir.join("2026-10.md")).unwrap();
    let backups = fs::read_dir(store.dir.join("backups")).unwrap().count();
    let filter = listing::Filter::new("  MeEtInG  ", Some(Status::Done)).unwrap();
    let view = listing::select_filtered(&store, None, date("2026-10-03"), &filter).unwrap();
    assert_eq!(view.days.len(), 3);
    assert_eq!(view.days[0].items.len(), 2); // Unfinished entries have moved to the active day.
    let text = listing::render_filtered(&view, date("2026-10-03"), &filter);
    assert!(text.contains("4 matching items"));
    assert!(text.contains("[1] Weekly meeting"));
    assert!(text.contains("[4] weekly meeting"));
    for (result, source_date, source_number) in [
        (1, "2026-09-30", 1),
        (2, "2026-09-30", 2),
        (3, "2026-10-01", 1),
        (4, "2026-10-02", 4),
    ] {
        let (source, number) = view.result(result, Some(&filter)).unwrap();
        assert_eq!(source.date, date(source_date));
        assert_eq!(number, source_number);
    }
    for result in [0, 5, usize::MAX] {
        assert!(view.result(result, Some(&filter)).is_err());
    }
    assert!(text.contains("[1] Weekly meeting"));
    assert!(text.contains("[2] weekly meeting"));
    assert!(text.contains("[3] WEEKLY meeting"));
    assert!(text.contains("[4] weekly meeting"));
    assert!(!text.contains("draft"));
    assert!(!text.contains("unrelated"));
    assert!(text.contains("make today active"));
    let scoped = listing::select_filtered(
        &store,
        Some("2026-10-01..2026-10-02"),
        date("2026-10-03"),
        &filter,
    )
    .unwrap();
    assert_eq!(scoped.days.len(), 2);
    let none =
        listing::select_filtered(&store, Some("2026-10-03"), date("2026-10-03"), &filter).unwrap();
    assert!(none.days.is_empty());
    assert!(
        listing::render_filtered(&none, date("2026-10-03"), &filter).contains("No matching items")
    );
    let status_only = listing::Filter::new("", Some(Status::Doing)).unwrap();
    assert_eq!(
        listing::select_filtered(&store, None, date("2026-10-03"), &status_only)
            .unwrap()
            .days
            .len(),
        1
    );
    assert_eq!(fs::read(store.dir.join("2026-10.md")).unwrap(), original);
    assert_eq!(
        fs::read_dir(store.dir.join("backups")).unwrap().count(),
        backups
    );
}

#[test]
fn filtering_empty_logs_is_read_only_and_search_rejects_control_characters() {
    let (_temp, store) = fixture();
    let filter = listing::Filter::new("möte 日本語", None).unwrap();
    let view = listing::select_filtered(&store, None, date("2026-10-03"), &filter).unwrap();
    assert!(view.days.is_empty());
    assert!(view.notice.contains("No matching items"));
    assert!(!store.dir.exists());
    for query in ["bad\ntext", "bad\ttext", "\u{1b}[2J"] {
        assert!(listing::Filter::new(query, None).is_err());
    }
}

#[test]
fn first_day_is_empty_and_starting_again_is_idempotent() {
    let (_temp, store) = fixture();
    assert!(store.new_day(date("2026-10-05")).unwrap());
    assert!(titles(&store).is_empty());
    assert!(
        listing::select(&store, None, date("2026-10-05"))
            .unwrap()
            .notice
            .contains("No previous day")
    );
    store
        .change(Change::Add("weekly meeting".into(), Status::Todo), None)
        .unwrap();
    let before = fs::read(store.dir.join("2026-10.md")).unwrap();
    assert!(!store.new_day(date("2026-10-05")).unwrap());
    assert_eq!(fs::read(store.dir.join("2026-10.md")).unwrap(), before);
}

#[test]
fn friday_to_monday_moves_todo_and_doing_and_leaves_only_done_across_months() {
    let (_temp, store) = fixture();
    store.new_day(date("2026-07-31")).unwrap();
    for title in ["todo", "in progress", "finished"] {
        store
            .change(Change::Add(title.into(), Status::Todo), None)
            .unwrap();
    }
    store.change(Change::Move(3, Status::Done), None).unwrap();
    store.change(Change::Move(2, Status::Doing), None).unwrap();
    store.new_day(date("2026-08-03")).unwrap();
    assert_eq!(
        titles(&store),
        [
            ("todo".into(), Status::Todo),
            ("in progress".into(), Status::Todo)
        ]
    );
    let friday = store.previous(date("2026-08-03")).unwrap().unwrap();
    assert_eq!(friday.items.len(), 1);
    assert_eq!(friday.items[0].title, "finished");
    assert_eq!(friday.items[0].status, Status::Done);
    assert_eq!(fs::read_dir(store.dir.join("backups")).unwrap().count(), 0);
    let monday = fs::read(store.dir.join("2026-08.md")).unwrap();
    store
        .change(Change::Add("new month item".into(), Status::Todo), None)
        .unwrap();
    assert_eq!(
        fs::read(store.dir.join("backups/2026-08.md.000001.bak")).unwrap(),
        monday
    );
    assert_eq!(
        store
            .days(None)
            .unwrap()
            .iter()
            .map(|d| d.date)
            .collect::<Vec<_>>(),
        [date("2026-07-31"), date("2026-08-03")]
    );
}

#[test]
fn mutations_target_last_started_day_and_duplicate_titles_are_independent() {
    let (_temp, store) = fixture();
    store.new_day(date("2026-10-02")).unwrap();
    for _ in 0..2 {
        store
            .change(Change::Add("weekly meeting".into(), Status::Todo), None)
            .unwrap();
    }
    store.change(Change::Move(2, Status::Doing), None).unwrap();
    store
        .change(Change::Edit(2, "second meeting".into()), None)
        .unwrap();
    let view = listing::select(&store, None, date("2026-10-05")).unwrap();
    assert_eq!(view.active, Some(date("2026-10-02")));
    assert!(listing::render(&view, date("2026-10-05")).contains("[2] second meeting"));
    store.change(Change::Move(2, Status::Done), None).unwrap();
    store.change(Change::Move(2, Status::Todo), None).unwrap();
    store.change(Change::Remove(1), None).unwrap();
    assert_eq!(titles(&store), [("second meeting".into(), Status::Todo)]);
    assert!(
        !fs::read_to_string(store.dir.join("2026-10.md"))
            .unwrap()
            .contains("# 2026-10-05")
    );
}

#[test]
fn empty_today_includes_previous_record_with_historical_letters() {
    let (_temp, store) = fixture();
    store.new_day(date("2026-10-02")).unwrap();
    store
        .change(Change::Add("finished".into(), Status::Todo), None)
        .unwrap();
    store.change(Change::Move(1, Status::Done), None).unwrap();
    store.new_day(date("2026-10-05")).unwrap();
    let view = listing::select(&store, None, date("2026-10-05")).unwrap();
    assert_eq!(view.days.len(), 2);
    let rendered = listing::render(&view, date("2026-10-05"));
    assert!(rendered.contains("2026-10-05 Monday (today; active"));
    assert!(rendered.contains("2026-10-02 Friday (history"));
    assert!(rendered.contains("[A] finished"));
    assert!(store.change(Change::Remove(1), None).is_err());
    assert!(
        store
            .previous(date("2026-10-05"))
            .unwrap()
            .unwrap()
            .items
            .iter()
            .any(|i| i.title == "finished")
    );
    let explicit = listing::select(&store, Some("today"), date("2026-10-05")).unwrap();
    assert_eq!(explicit.days.len(), 1);
}

#[test]
fn selectors_have_calendar_boundaries_and_no_fallback() {
    let today = date("2026-10-07");
    assert_eq!(
        listing::range("week", today).unwrap(),
        (date("2026-10-05"), date("2026-10-11"))
    );
    assert_eq!(
        listing::range("month", today).unwrap(),
        (date("2026-10-01"), date("2026-10-31"))
    );
    assert_eq!(
        listing::range("yesterday", today).unwrap(),
        (date("2026-10-06"), date("2026-10-06"))
    );
    assert_eq!(
        listing::range("month", date("2024-02-29")).unwrap().1,
        date("2024-02-29")
    );
    assert_eq!(
        listing::range("month", date("2026-12-15")).unwrap().1,
        date("2026-12-31")
    );
    assert!(listing::range("2026-10-07..2026-10-01", today).is_err());
    assert!(listing::range("2026-02-30", today).is_err());
    let (_temp, store) = fixture();
    store.new_day(date("2026-09-30")).unwrap();
    store.new_day(date("2026-10-02")).unwrap();
    let range = listing::select(&store, Some("2026-09-30..2026-10-02"), today).unwrap();
    assert_eq!(range.days.len(), 2);
    let yesterday = listing::select(&store, Some("yesterday"), today).unwrap();
    assert!(yesterday.days.is_empty());
    assert!(listing::render(&yesterday, today).contains("No recorded days for yesterday"));
}

#[test]
fn comments_blank_lines_crlf_and_previous_days_survive_mutations() {
    let (_temp, store) = fixture();
    fs::create_dir_all(&store.dir).unwrap();
    let prefix = "<!-- my notes\r\n# this is not a date\r\n-->\r\n# 2026-10-01\r\n\r\n## To do\r\n\r\n## Doing\r\n\r\n## Done\r\n- finished\r\n\r\n";
    let active = "# 2026-10-02\r\n\r\n## To do\r\n<!-- keep this comment -->\r\n- meeting\r\n\r\n## Doing\r\n\r\n## Done";
    fs::write(store.dir.join("2026-10.md"), format!("{prefix}{active}")).unwrap();
    store.change(Change::Move(1, Status::Doing), None).unwrap();
    store
        .change(Change::Edit(1, "möte 日本語".into()), None)
        .unwrap();
    store
        .change(Change::Add("new title".into(), Status::Todo), None)
        .unwrap();
    let result = fs::read_to_string(store.dir.join("2026-10.md")).unwrap();
    assert!(result.starts_with(&format!("<!-- kiroku-format: 1 -->\r\n\r\n{prefix}")));
    assert!(result.contains("<!-- keep this comment -->\r\n"));
    assert!(result.contains("- möte 日本語\r\n"));
    assert!(!result.replace("\r\n", "").contains('\n'));
    assert_eq!(store.active().unwrap().unwrap().items.len(), 2);
}

#[test]
fn malformed_files_are_never_overwritten() {
    let cases = [
        "# 2026-10-02\n## To do\n- title\n## Doing\n", // missing section
        "# 2026-10-02\n## To do\n## Doing\n## Done\nunrecognized prose\n",
        "# 2026-10-02\n## To do\n## To do\n## Doing\n## Done\n",
        "# 2026-10-02\n## To do\n## Doing\n## Done\n<!-- unclosed\n",
        "# 2026-09-02\n## To do\n## Doing\n## Done\n", // wrong monthly file
        "# 2026-10-02\n## To do\n- \n## Doing\n## Done\n",
        "# 2026-10-02\n## To do\n## Doing\n## Done\n# 2026-10-02\n## To do\n## Doing\n## Done\n",
    ];
    for text in cases {
        let (_temp, store) = fixture();
        fs::create_dir_all(&store.dir).unwrap();
        let path = store.dir.join("2026-10.md");
        fs::write(&path, text).unwrap();
        assert!(
            store
                .change(Change::Add("new".into(), Status::Todo), None)
                .is_err(),
            "{text}"
        );
        assert!(store.new_day(date("2026-10-03")).is_err(), "{text}");
        assert_eq!(fs::read_to_string(path).unwrap(), text);
    }
}

#[test]
fn bad_numbers_and_titles_do_not_modify_log() {
    let (_temp, store) = fixture();
    store.new_day(date("2026-10-02")).unwrap();
    store
        .change(Change::Add("meeting".into(), Status::Todo), None)
        .unwrap();
    let path = store.dir.join("2026-10.md");
    let before = fs::read(&path).unwrap();
    for change in [
        Change::Remove(0),
        Change::Remove(2),
        Change::Add(" \t ".into(), Status::Todo),
        Change::Edit(1, "two\nlines".into()),
        Change::Add("bad\u{1b}[2Jtitle".into(), Status::Todo),
    ] {
        assert!(store.change(change, None).is_err());
        assert_eq!(fs::read(&path).unwrap(), before);
    }
}

#[test]
fn stale_tui_snapshot_does_not_edit_the_wrong_item() {
    let (_temp, store) = fixture();
    store.new_day(date("2026-10-02")).unwrap();
    store
        .change(Change::Add("meeting".into(), Status::Todo), None)
        .unwrap();
    let old = store.active().unwrap().unwrap();
    store
        .change(Change::Edit(1, "changed externally".into()), None)
        .unwrap();
    assert!(store.change(Change::Remove(1), Some(&old)).is_err());
    assert_eq!(titles(&store)[0].0, "changed externally");
}

#[test]
fn concurrent_additions_are_serialized_without_lost_items() {
    let (_temp, store) = fixture();
    store.new_day(date("2026-10-02")).unwrap();
    let threads: Vec<_> = (0..12)
        .map(|i| {
            let store = store.clone();
            std::thread::spawn(move || {
                store
                    .change(Change::Add(format!("item {i}"), Status::Todo), None)
                    .unwrap()
            })
        })
        .collect();
    for thread in threads {
        thread.join().unwrap();
    }
    assert_eq!(titles(&store).len(), 12);
    assert_eq!(fs::read_dir(store.dir.join("backups")).unwrap().count(), 5);
    let newest = fs::read_to_string(store.dir.join("backups/2026-10.md.000012.bak")).unwrap();
    assert_eq!(
        newest
            .lines()
            .filter(|line| line.starts_with("- item "))
            .count(),
        11
    );
}

#[test]
fn filtered_listing_does_not_parse_unrelated_old_files() {
    let (_temp, store) = fixture();
    store.new_day(date("2026-10-02")).unwrap();
    fs::write(store.dir.join("2025-01.md"), "bad old file").unwrap();
    fs::write(store.dir.join("日月火.md"), "unrelated").unwrap();
    assert_eq!(
        listing::select(&store, Some("month"), date("2026-10-05"))
            .unwrap()
            .days
            .len(),
        1
    );
}

#[test]
fn configuration_defaults_saved_path_override_and_home_expansion() {
    let home = tempfile::tempdir().unwrap();
    let config_dir = home.path().join("config");
    assert_eq!(
        config::resolve_with(None, home.path(), &config_dir).unwrap(),
        home.path().join("Documents/kiroku")
    );
    fs::create_dir_all(config_dir.join("kiroku")).unwrap();
    let config_path = config_dir.join("kiroku/config.toml");
    fs::write(&config_path, "data_dir = '~/my logs'\n").unwrap();
    assert_eq!(
        config::resolve_with(None, home.path(), &config_dir).unwrap(),
        home.path().join("my logs")
    );
    let override_path = home.path().join("override");
    assert_eq!(
        config::resolve_with(Some(&override_path), home.path(), &config_dir).unwrap(),
        override_path
    );
    fs::write(&config_path, "data_dir = 123").unwrap();
    assert!(config::resolve_with(None, home.path(), &config_dir).is_err());
    assert!(config::resolve_with(Some(&override_path), home.path(), &config_dir).is_ok());
    assert!(config::expand_home(std::path::Path::new("~other/logs"), home.path()).is_err());
}

#[test]
fn letter_labels_extend_past_z_and_stay_unique_across_days() {
    assert_eq!(listing::letters(0), "A");
    assert_eq!(listing::letters(25), "Z");
    assert_eq!(listing::letters(26), "AA");
    assert_eq!(listing::letters(51), "AZ");
    assert_eq!(listing::letters(52), "BA");
    let (_temp, store) = fixture();
    for n in 0..3 {
        store
            .new_day(date("2026-10-01") + Duration::days(n))
            .unwrap();
        store
            .change(Change::Add("done".into(), Status::Todo), None)
            .unwrap();
        store.change(Change::Move(1, Status::Done), None).unwrap();
    }
    let rendered = listing::render(
        &listing::select(&store, Some("month"), date("2026-10-03")).unwrap(),
        date("2026-10-03"),
    );
    assert!(rendered.contains("[A] done"));
    assert!(rendered.contains("[B] done"));
    assert!(rendered.contains("[1] done"));
}

#[test]
fn reorder_preserves_sections_comments_line_endings_and_rejects_stale_views() {
    for newline in ["\n", "\r\n"] {
        let (_temp, store) = fixture();
        fs::create_dir_all(&store.dir).unwrap();
        let path = store.dir.join("2026-10.md");
        let original = "<!-- kiroku-format: 1 -->\n# 2026-10-01\n## To do\n- duplicate\n<!-- keep here -->\n\n- duplicate\n- last todo  \n## Doing\n- first doing\n- last doing\n## Done\n- first done\n- last done".replace('\n', newline);
        fs::write(&path, &original).unwrap();
        let initial = store.active().unwrap().unwrap();
        for (number, direction) in [
            (1, Direction::Up),
            (3, Direction::Down),
            (4, Direction::Up),
            (5, Direction::Down),
            (6, Direction::Up),
            (7, Direction::Down),
        ] {
            store
                .change(Change::Reorder(number, direction, 1), Some(&initial))
                .unwrap();
            assert_eq!(fs::read_to_string(&path).unwrap(), original);
            assert!(!store.dir.join("backups").exists());
        }
        // Duplicate titles are distinct occurrences, selected by number.
        store
            .change(Change::Reorder(2, Direction::Down, 1), Some(&initial))
            .unwrap();
        let expected = original.replacen(
            &format!("- duplicate{newline}- last todo  "),
            &format!("- last todo  {newline}- duplicate"),
            1,
        );
        assert_eq!(fs::read_to_string(&path).unwrap(), expected);
        assert!(
            store
                .change(Change::Reorder(1, Direction::Down, 1), Some(&initial))
                .is_err()
        );
        store
            .change(Change::Reorder(4, Direction::Down, 1), None)
            .unwrap();
        store
            .change(Change::Reorder(7, Direction::Up, 1), None)
            .unwrap();
        assert_eq!(
            titles(&store)
                .iter()
                .map(|(title, _)| title.as_str())
                .collect::<Vec<_>>(),
            [
                "duplicate",
                "last todo",
                "duplicate",
                "last doing",
                "first doing",
                "last done",
                "first done"
            ]
        );
        let saved = fs::read_to_string(&path).unwrap();
        assert!(saved.contains(&format!("<!-- keep here -->{newline}{newline}")));
        assert!(saved.ends_with("- first done"));
        for number in [0, 8, usize::MAX] {
            assert!(
                store
                    .change(Change::Reorder(number, Direction::Down, 1), None)
                    .is_err()
            );
            assert_eq!(fs::read_to_string(&path).unwrap(), saved);
        }
    }
}

#[test]
fn reorder_multiple_steps_is_one_save_and_preserves_intervening_order() {
    for status in Status::ALL {
        let (_temp, store) = fixture();
        store.new_day(chrono::Local::now().date_naive()).unwrap();
        for section in Status::ALL {
            for title in ["first", "second", "third", "fourth"] {
                store
                    .change(Change::Add(title.into(), section), None)
                    .unwrap();
            }
        }
        let day = store.active().unwrap().unwrap();
        let start = day
            .numbered()
            .iter()
            .position(|item| item.status == status)
            .unwrap();
        let path = store.dir.join(format!("{}.md", day.date.format("%Y-%m")));
        let original = fs::read(&path).unwrap();
        let backups = store.dir.join("backups");
        let latest_before = fs::read_dir(&backups)
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .max()
            .unwrap();
        let changed = store
            .change(Change::Reorder(start + 1, Direction::Down, 2), Some(&day))
            .unwrap();
        assert_eq!(
            changed
                .numbered()
                .iter()
                .filter(|item| item.status == status)
                .map(|item| item.title.as_str())
                .collect::<Vec<_>>(),
            ["second", "third", "first", "fourth"]
        );
        let latest_after = fs::read_dir(&backups)
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .max()
            .unwrap();
        let sequence = |name: &std::ffi::OsStr| {
            name.to_str()
                .unwrap()
                .split('.')
                .nth(2)
                .unwrap()
                .parse::<usize>()
                .unwrap()
        };
        assert_eq!(sequence(&latest_after), sequence(&latest_before) + 1);
        assert_eq!(fs::read(backups.join(latest_after)).unwrap(), original);
        let changed = store
            .change(Change::Reorder(start + 3, Direction::Up, usize::MAX), None)
            .unwrap();
        assert_eq!(changed.numbered()[start].title, "first");
        let changed = store
            .change(
                Change::Reorder(start + 1, Direction::Down, usize::MAX),
                None,
            )
            .unwrap();
        assert_eq!(changed.numbered()[start + 3].title, "first");
        let saved = fs::read(&path).unwrap();
        assert!(
            store
                .change(Change::Reorder(start + 1, Direction::Down, 0), None)
                .is_err()
        );
        assert_eq!(fs::read(&path).unwrap(), saved);
        for section in Status::ALL.into_iter().filter(|section| *section != status) {
            assert_eq!(
                changed
                    .numbered()
                    .iter()
                    .filter(|item| item.status == section)
                    .map(|item| item.title.as_str())
                    .collect::<Vec<_>>(),
                ["first", "second", "third", "fourth"]
            );
        }
    }
}
