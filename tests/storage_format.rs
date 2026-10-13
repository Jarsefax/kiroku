use kiroku::{
    listing,
    storage::{Change, Status, Store, parse_date},
};
use std::fs;

const LEGACY: &str = include_str!("fixtures/storage/legacy-v1.md");
const HEADER: &str = "<!-- kiroku-format: 1 -->\n\n";

#[test]
fn legacy_and_versioned_logs_preserve_bytes_until_an_actual_change() {
    for newline in ["\n", "\r\n"] {
        for header in [
            "",
            HEADER,
            "\n<!-- notes -->\n  <!-- kiroku-format: 1 -->  \n",
        ] {
            // Also exercise a manually edited file without a final newline.
            for final_newline in [true, false] {
                let temp = tempfile::tempdir().unwrap();
                let store = Store::new(temp.path().to_path_buf());
                let path = store.dir.join("2026-10.md");
                let text = format!("{header}{LEGACY}").replace('\n', newline);
                let text = if final_newline {
                    text
                } else {
                    text.trim_end_matches(['\r', '\n']).to_owned()
                };
                fs::write(&path, &text).unwrap();

                let days = store.days(None).unwrap();
                assert_eq!(days.len(), 2);
                let day = store.active().unwrap().unwrap();
                assert_eq!(day.date, parse_date("2026-10-06").unwrap());
                assert_eq!(
                    day.numbered()
                        .iter()
                        .map(|item| (item.title.as_str(), item.status))
                        .collect::<Vec<_>>(),
                    [
                        ("plan café 日本語", Status::Todo),
                        ("duplicate", Status::Todo),
                        ("duplicate", Status::Doing),
                        ("finished", Status::Done),
                    ]
                );
                listing::select(&store, None, day.date).unwrap();
                listing::select_filtered(
                    &store,
                    None,
                    day.date,
                    &listing::Filter::new("duplicate", None).unwrap(),
                )
                .unwrap();
                assert_eq!(store.previous(day.date).unwrap().unwrap().items.len(), 2);
                assert!(!store.new_day(day.date).unwrap());
                store.change(Change::Move(1, Status::Todo), None).unwrap();
                store
                    .change(Change::Edit(1, "plan café 日本語".into()), None)
                    .unwrap();
                assert_eq!(fs::read_to_string(&path).unwrap(), text);
                assert!(!store.dir.join("backups").exists());

                // Use the snapshot from before annotation to guard the edit.
                store
                    .change(Change::Edit(1, "renamed".into()), Some(&day))
                    .unwrap();
                let expected = text.replace("- plan café 日本語", "- renamed");
                let expected = if header.is_empty() {
                    format!("{}{expected}", HEADER.replace('\n', newline))
                } else {
                    expected
                };
                assert_eq!(fs::read_to_string(&path).unwrap(), expected);
                assert_eq!(
                    fs::read_to_string(store.dir.join("backups/2026-10.md.000001.bak")).unwrap(),
                    text
                );
                assert_eq!(
                    store.active().unwrap().unwrap().numbered()[0].title,
                    "renamed"
                );
            }
        }
    }
}

#[test]
fn new_months_have_one_header_and_rollover_preserves_done_and_older_history() {
    let temp = tempfile::tempdir().unwrap();
    let store = Store::new(temp.path().to_path_buf());
    let old = store.dir.join("2026-10.md");
    fs::write(&old, LEGACY).unwrap();
    store.new_day(parse_date("2026-11-02").unwrap()).unwrap();
    let expected = format!(
        "{HEADER}{}",
        LEGACY
            .replace("- plan café 日本語\n", "")
            .replace("- duplicate\n", "")
    );
    assert_eq!(fs::read_to_string(&old).unwrap(), expected);
    let days = store.days(None).unwrap();
    assert_eq!(days[0].items.len(), 2); // Earlier history is unchanged.
    assert_eq!(days[1].items.len(), 1);
    assert_eq!(days[1].items[0].status, Status::Done);
    assert_eq!(days[2].items.len(), 3);
    assert!(days[2].items.iter().all(|item| item.status == Status::Todo));
    let path = store.dir.join("2026-11.md");
    assert!(fs::read_to_string(&path).unwrap().starts_with(HEADER));
    store.new_day(parse_date("2026-11-03").unwrap()).unwrap();
    store
        .add_on_date(parse_date("2026-11-03").unwrap(), "new", Status::Done)
        .unwrap();
    store
        .repeat_on_date(
            parse_date("2026-11-03").unwrap(),
            Some(parse_date("2026-11-03").unwrap()),
            1,
            Status::Todo,
            None,
        )
        .unwrap();
    assert_eq!(
        fs::read_to_string(&path)
            .unwrap()
            .matches("<!-- kiroku-format:")
            .count(),
        1
    );
    assert_eq!(store.days(None).unwrap().len(), 4);
}

#[test]
fn unsupported_and_invalid_headers_block_reads_and_writes_without_backups() {
    for (text, expected_error) in [
        (
            format!("<!-- kiroku-format: 2 -->\n{LEGACY}"),
            "Upgrade Kiroku",
        ),
        // Version detection must run before attempting the current body grammar.
        (
            "<!-- kiroku-format: 99 -->\nfuture body syntax\n".into(),
            "Upgrade Kiroku",
        ),
        (
            format!("{HEADER}{HEADER}{LEGACY}"),
            "duplicate kiroku-format",
        ),
        (format!("{LEGACY}{HEADER}"), "must precede the first date"),
        (
            format!("<!-- kiroku-format: 0 -->\n{LEGACY}"),
            "Invalid kiroku-format",
        ),
        (
            format!("<!-- kiroku-format: -1 -->\n{LEGACY}"),
            "Invalid kiroku-format",
        ),
        (
            format!("<!-- kiroku-format: +1 -->\n{LEGACY}"),
            "Invalid kiroku-format",
        ),
        (
            format!("<!-- kiroku-format: one -->\n{LEGACY}"),
            "Invalid kiroku-format",
        ),
        (
            format!("<!-- kiroku-format: -->\n{LEGACY}"),
            "Invalid kiroku-format",
        ),
        (
            format!("<!-- kiroku-format 1 -->\n{LEGACY}"),
            "Invalid kiroku-format",
        ),
        (
            format!("<!-- kiroku-format: 1\n-->\n{LEGACY}"),
            "Invalid kiroku-format",
        ),
        (
            format!("<!-- kiroku-format: 1 --> extra\n{LEGACY}"),
            "Invalid kiroku-format",
        ),
        (
            format!("<!-- kiroku-format: 999999999999999999999 -->\n{LEGACY}"),
            "Invalid kiroku-format",
        ),
    ] {
        let temp = tempfile::tempdir().unwrap();
        let store = Store::new(temp.path().to_path_buf());
        let path = store.dir.join("2026-10.md");
        fs::write(&path, &text).unwrap();
        for operation in 0..10 {
            let result = match operation {
                0 => store.active().map(|_| ()),
                1 => store.days(None).map(|_| ()),
                2 => store
                    .previous(parse_date("2026-11-02").unwrap())
                    .map(|_| ()),
                3 => store.new_day(parse_date("2026-11-02").unwrap()).map(|_| ()),
                4 => store
                    .change(Change::Add("new".into(), Status::Todo), None)
                    .map(|_| ()),
                5 => store
                    .change(Change::Move(1, Status::Done), None)
                    .map(|_| ()),
                6 => store
                    .change(Change::Edit(1, "new".into()), None)
                    .map(|_| ()),
                7 => store.change(Change::Remove(1), None).map(|_| ()),
                8 => store.repeat(None, 1, Status::Todo, None, None).map(|_| ()),
                _ => store
                    .repeat(
                        Some(parse_date("2026-10-05").unwrap()),
                        1,
                        Status::Todo,
                        None,
                        None,
                    )
                    .map(|_| ()),
            };
            let message = format!("{:#}", result.unwrap_err());
            assert!(message.contains(expected_error), "{operation}: {message}");
            assert!(message.contains("2026-10.md:"), "{message}");
            assert_eq!(fs::read_to_string(&path).unwrap(), text);
            assert!(!store.dir.join("2026-11.md").exists());
            assert!(!store.dir.join("backups").exists());
        }
    }
}

#[test]
fn unsupported_historical_source_cannot_be_copied_into_a_supported_file() {
    let temp = tempfile::tempdir().unwrap();
    let store = Store::new(temp.path().to_path_buf());
    let source = store.dir.join("2026-10.md");
    let contents = format!("<!-- kiroku-format: 2 -->\n{LEGACY}");
    fs::write(&source, &contents).unwrap();
    let destination = store.dir.join("2026-11.md");
    let original = format!("{HEADER}# 2026-11-02\n\n## To do\n\n## Doing\n\n## Done\n");
    fs::write(&destination, &original).unwrap();
    let error = store
        .repeat(
            Some(parse_date("2026-10-06").unwrap()),
            1,
            Status::Todo,
            None,
            None,
        )
        .unwrap_err();
    assert!(format!("{error:#}").contains("Upgrade Kiroku"));
    assert_eq!(fs::read_to_string(source).unwrap(), contents);
    assert_eq!(fs::read_to_string(destination).unwrap(), original);
    assert!(!store.dir.join("backups").exists());
}

#[test]
fn unsupported_empty_eligible_month_cannot_be_skipped_to_find_an_older_active_day() {
    let temp = tempfile::tempdir().unwrap();
    let store = Store::new(temp.path().to_path_buf());
    let legacy = LEGACY.replace("2026-10-", "2026-09-");
    fs::write(store.dir.join("2026-09.md"), &legacy).unwrap();
    let path = store.dir.join("2026-10.md");
    fs::write(&path, "<!-- kiroku-format: 2 -->\n").unwrap();
    let error = store
        .change(Change::Add("new".into(), Status::Todo), None)
        .unwrap_err();
    assert!(format!("{error:#}").contains("Upgrade Kiroku"));
    assert_eq!(
        fs::read_to_string(store.dir.join("2026-09.md")).unwrap(),
        legacy
    );
    assert_eq!(
        fs::read_to_string(path).unwrap(),
        "<!-- kiroku-format: 2 -->\n"
    );
    assert!(!store.dir.join("backups").exists());
}
