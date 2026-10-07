use chrono::{Duration, NaiveDate};
use kiroku::{
    config, listing,
    storage::{Change, Status, Store, parse_date},
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
fn reading_empty_store_does_not_create_anything() {
    let (_temp, store) = fixture();
    assert!(store.active().unwrap().is_none());
    let view = listing::select(&store, None, date("2026-10-05")).unwrap();
    assert!(view.days.is_empty());
    assert!(view.notice.contains("No recorded days"));
    assert!(!store.dir.exists());
    assert!(store.change(Change::Add("meeting".into()), None).is_err());
    assert!(store.active().unwrap().is_none());
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
        .change(Change::Add("weekly meeting".into()), None)
        .unwrap();
    let before = fs::read(store.dir.join("2026-10.md")).unwrap();
    assert!(!store.new_day(date("2026-10-05")).unwrap());
    assert_eq!(fs::read(store.dir.join("2026-10.md")).unwrap(), before);
}

#[test]
fn friday_to_monday_carries_todo_and_doing_but_not_done_across_months() {
    let (_temp, store) = fixture();
    store.new_day(date("2026-07-31")).unwrap();
    for title in ["todo", "in progress", "finished"] {
        store.change(Change::Add(title.into()), None).unwrap();
    }
    store.change(Change::Move(3, Status::Done), None).unwrap();
    store.change(Change::Move(2, Status::Doing), None).unwrap();
    let friday = fs::read(store.dir.join("2026-07.md")).unwrap();
    store.new_day(date("2026-08-03")).unwrap();
    assert_eq!(
        titles(&store),
        [
            ("todo".into(), Status::Todo),
            ("in progress".into(), Status::Todo)
        ]
    );
    assert_eq!(fs::read(store.dir.join("2026-07.md")).unwrap(), friday);
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
            .change(Change::Add("weekly meeting".into()), None)
            .unwrap();
    }
    store.change(Change::Move(2, Status::Doing), None).unwrap();
    store
        .change(Change::Edit(2, "second meeting".into()), None)
        .unwrap();
    let view = listing::select(&store, None, date("2026-10-05")).unwrap();
    assert_eq!(view.active, Some(date("2026-10-02")));
    assert!(listing::render(&view, date("2026-10-05")).contains("2. second meeting"));
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
    store.change(Change::Add("finished".into()), None).unwrap();
    store.change(Change::Move(1, Status::Done), None).unwrap();
    store.new_day(date("2026-10-05")).unwrap();
    let view = listing::select(&store, None, date("2026-10-05")).unwrap();
    assert_eq!(view.days.len(), 2);
    let rendered = listing::render(&view, date("2026-10-05"));
    assert!(rendered.contains("2026-10-05 (today; active"));
    assert!(rendered.contains("2026-10-02 (history"));
    assert!(rendered.contains("A. finished"));
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
    store.change(Change::Add("new title".into()), None).unwrap();
    let result = fs::read_to_string(store.dir.join("2026-10.md")).unwrap();
    assert!(result.starts_with(prefix));
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
            store.change(Change::Add("new".into()), None).is_err(),
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
    store.change(Change::Add("meeting".into()), None).unwrap();
    let path = store.dir.join("2026-10.md");
    let before = fs::read(&path).unwrap();
    for change in [
        Change::Remove(0),
        Change::Remove(2),
        Change::Add(" \t ".into()),
        Change::Edit(1, "two\nlines".into()),
        Change::Add("bad\u{1b}[2Jtitle".into()),
    ] {
        assert!(store.change(change, None).is_err());
        assert_eq!(fs::read(&path).unwrap(), before);
    }
    assert!(store.new_day(date("2026-10-01")).is_err());
    assert_eq!(fs::read(&path).unwrap(), before);
}

#[test]
fn stale_tui_snapshot_does_not_edit_the_wrong_item() {
    let (_temp, store) = fixture();
    store.new_day(date("2026-10-02")).unwrap();
    store.change(Change::Add("meeting".into()), None).unwrap();
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
                    .change(Change::Add(format!("item {i}")), None)
                    .unwrap()
            })
        })
        .collect();
    for thread in threads {
        thread.join().unwrap();
    }
    assert_eq!(titles(&store).len(), 12);
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
        home.path().join("Document/kiroku")
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
        store.change(Change::Add("done".into()), None).unwrap();
        store.change(Change::Move(1, Status::Done), None).unwrap();
    }
    let rendered = listing::render(
        &listing::select(&store, Some("month"), date("2026-10-03")).unwrap(),
        date("2026-10-03"),
    );
    assert!(rendered.contains("A. done"));
    assert!(rendered.contains("B. done"));
    assert!(rendered.contains("1. done"));
}
