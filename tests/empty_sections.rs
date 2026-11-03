use kiroku::storage::{Status, Store, parse_date};
use std::fs;

fn date(text: &str) -> chrono::NaiveDate {
    parse_date(text).unwrap()
}

fn sparse_day(day: &str, mask: usize) -> String {
    let mut text = format!("<!-- kiroku-format: 2 -->\n\n# {day}\n\n<!-- day note -->\n");
    for (index, status) in Status::ALL.into_iter().enumerate() {
        if mask & (1 << index) != 0 {
            text.push_str(&format!(
                "\n## {}\n- original {}\n",
                status.label(),
                status.label()
            ));
        }
    }
    text
}

#[test]
fn rollover_cleans_only_the_closed_day_and_preserves_history_and_comments() {
    for previous in ["2026-10-02", "2026-09-30"] {
        for newline in ["\n", "\r\n"] {
            let temp = tempfile::tempdir().unwrap();
            let store = Store::new(temp.path().to_path_buf());
            let older_date = date(previous) - chrono::Duration::days(1);
            let older = format!(
                "# {older_date}\n\n## To do\n- older pending\n\n## Doing\n\n## Done\n- older done\n\n"
            );
            let source = format!(
                "# {previous}\n\n## To do\n- pending\n<!-- todo note\n## Doing\n-->\n\n## Doing\n- ongoing\n\n## Done\n<!-- done note -->\n\n"
            );
            let original =
                format!("<!-- kiroku-format: 1 -->\n\n{older}{source}").replace('\n', newline);
            let path = store
                .dir
                .join(format!("{}.md", date(previous).format("%Y-%m")));
            fs::write(&path, &original).unwrap();

            assert!(store.new_day(date("2026-10-05")).unwrap());
            let compact = format!(
                "# {previous}\n\n<!-- todo note\n## Doing\n-->\n\n## Doing\n- ongoing\n\n<!-- done note -->\n\n"
            );
            let expected =
                format!("<!-- kiroku-format: 2 -->\n\n{older}{compact}").replace('\n', newline);
            let saved = fs::read_to_string(&path).unwrap();
            if previous == "2026-09-30" {
                assert_eq!(saved, expected);
            } else {
                assert!(saved.starts_with(&expected));
                assert_eq!(
                    fs::read_to_string(store.dir.join("backups/2026-10.md.000001.bak")).unwrap(),
                    original
                );
            }
            if newline == "\r\n" {
                assert!(!saved.replace("\r\n", "").contains('\n'));
            }
            let days = store.days(None).unwrap();
            assert_eq!(days[0].items.len(), 2);
            assert_eq!(days[1].items.len(), 1);
            assert_eq!(days[1].items[0].status, Status::Doing);
            assert_eq!(
                days[2]
                    .items
                    .iter()
                    .map(|item| item.title.as_str())
                    .collect::<Vec<_>>(),
                ["pending", "ongoing"]
            );
            let destination = store.dir.join("2026-10.md");
            let destination_bytes = fs::read(&destination).unwrap();
            assert!(!store.new_day(date("2026-10-05")).unwrap());
            assert_eq!(fs::read(path).unwrap(), saved.as_bytes());
            assert_eq!(fs::read(destination).unwrap(), destination_bytes);
        }
    }
}

#[test]
fn an_empty_or_todo_only_closed_day_keeps_its_date_and_notes_without_headings() {
    for pending in ["", "- pending\n"] {
        for newline in ["\n", "\r\n"] {
            let temp = tempfile::tempdir().unwrap();
            let store = Store::new(temp.path().to_path_buf());
            let path = store.dir.join("2026-09.md");
            let source = format!(
                "# 2026-09-30\n\n## To do\n{pending}\n## Doing\n\n## Done\n<!-- note\n## To do\n-->"
            )
            .replace('\n', newline);
            fs::write(&path, &source).unwrap();
            store.new_day(date("2026-10-02")).unwrap();
            let expected = "<!-- kiroku-format: 2 -->\n\n# 2026-09-30\n\n<!-- note\n## To do\n-->"
                .replace('\n', newline);
            assert_eq!(fs::read_to_string(&path).unwrap(), expected);
            assert!(
                store
                    .previous(date("2026-10-02"))
                    .unwrap()
                    .unwrap()
                    .items
                    .is_empty()
            );
            let active = store.active_on(date("2026-10-02")).unwrap().unwrap();
            assert_eq!(active.items.len(), usize::from(!pending.is_empty()));
        }
    }
}

#[test]
fn an_already_compact_cross_month_source_is_not_rewritten() {
    let temp = tempfile::tempdir().unwrap();
    let store = Store::new(temp.path().to_path_buf());
    let path = store.dir.join("2026-09.md");
    let source = sparse_day("2026-09-30", 0b110);
    fs::write(&path, &source).unwrap();
    store.new_day(date("2026-10-02")).unwrap();
    assert_eq!(fs::read_to_string(path).unwrap(), source);
    assert!(!store.dir.join("backups").exists());
}

#[test]
fn additions_recreate_only_the_needed_section_for_every_sparse_day_shape() {
    for mask in 0..8 {
        for (target_index, target) in Status::ALL.into_iter().enumerate() {
            for newline in ["\n", "\r\n"] {
                for final_newline in [true, false] {
                    let temp = tempfile::tempdir().unwrap();
                    let store = Store::new(temp.path().to_path_buf());
                    let path = store.dir.join("2026-10.md");
                    let source = sparse_day("2026-10-02", mask).replace('\n', newline);
                    let source = if final_newline {
                        source
                    } else {
                        source.trim_end_matches(['\r', '\n']).to_owned()
                    };
                    fs::write(&path, &source).unwrap();
                    assert!(
                        store
                            .add_on_date_at(date("2026-10-02"), "invalid", target, Some(99))
                            .is_err()
                    );
                    assert_eq!(fs::read_to_string(&path).unwrap(), source);
                    assert!(!store.dir.join("backups").exists());

                    let updated = store
                        .add_on_date_at(date("2026-10-02"), "inserted 日本語", target, Some(1))
                        .unwrap();
                    assert_eq!(updated.items.len(), mask.count_ones() as usize + 1);
                    assert_eq!(
                        updated
                            .items
                            .iter()
                            .find(|item| item.status == target)
                            .unwrap()
                            .title,
                        "inserted 日本語"
                    );
                    let saved = fs::read_to_string(&path).unwrap();
                    assert!(saved.contains("<!-- day note -->"));
                    for (index, status) in Status::ALL.into_iter().enumerate() {
                        let present = mask & (1 << index) != 0;
                        assert_eq!(
                            saved.contains(&format!("## {}", status.label())),
                            present || index == target_index
                        );
                        if present {
                            assert!(saved.contains(&format!("- original {}", status.label())));
                        }
                    }
                    if newline == "\r\n" {
                        assert!(!saved.replace("\r\n", "").contains('\n'));
                    }
                    assert_eq!(
                        fs::read_to_string(store.dir.join("backups/2026-10.md.000001.bak"))
                            .unwrap(),
                        source
                    );
                    assert_eq!(
                        store.days(None).unwrap()[0].items.len(),
                        updated.items.len()
                    );
                }
            }
        }
    }
}

#[test]
fn moves_and_copies_can_target_an_omitted_section_within_and_across_months() {
    for previous in ["2026-10-02", "2026-09-30"] {
        for copy in [true, false] {
            let temp = tempfile::tempdir().unwrap();
            let store = Store::new(temp.path().to_path_buf());
            let path = store
                .dir
                .join(format!("{}.md", date(previous).format("%Y-%m")));
            let source = sparse_day(previous, 0b010);
            fs::write(&path, &source).unwrap();
            if previous == "2026-09-30" {
                fs::write(store.dir.join("2026-10.md"), sparse_day("2026-10-02", 0)).unwrap();
            }
            let day = if copy {
                store.repeat_on_date_at(
                    date("2026-10-02"),
                    Some(date(previous)),
                    1,
                    Status::Todo,
                    None,
                    Some(1),
                )
            } else {
                store.move_on_date_at(
                    date("2026-10-02"),
                    Some(date(previous)),
                    1,
                    Status::Todo,
                    None,
                    Some(1),
                )
            }
            .unwrap();
            assert_eq!(day.numbered()[0].title, "original Doing");
            assert_eq!(day.numbered()[0].status, Status::Todo);
            let history = store
                .days(Some((date(previous), date(previous))))
                .unwrap()
                .remove(0);
            assert_eq!(
                history
                    .items
                    .iter()
                    .filter(|item| item.status == Status::Doing)
                    .count(),
                usize::from(copy)
            );
            if copy && previous == "2026-09-30" {
                assert_eq!(fs::read_to_string(path).unwrap(), source);
            }
        }
    }
}

#[test]
fn format_two_still_rejects_duplicate_out_of_order_and_orphaned_items() {
    for body in [
        "## To do\n## To do\n",
        "## Done\n## Doing\n",
        "- orphan\n",
        "## Doing\n- older\n# 2026-10-03\n- orphan\n",
        "## Unknown\n",
        "<!-- unclosed\n",
    ] {
        let temp = tempfile::tempdir().unwrap();
        let store = Store::new(temp.path().to_path_buf());
        let path = store.dir.join("2026-10.md");
        let source = format!("<!-- kiroku-format: 2 -->\n# 2026-10-02\n{body}");
        fs::write(&path, &source).unwrap();
        assert!(store.days(None).is_err(), "{body}");
        assert!(
            store
                .add_on_date(date("2026-10-02"), "new", Status::Todo)
                .is_err()
        );
        assert_eq!(fs::read_to_string(path).unwrap(), source);
        assert!(!store.dir.join("backups").exists());
    }
}

#[test]
fn cleanup_only_rollover_aborts_before_changing_logs_if_a_backup_cannot_be_made() {
    for previous in ["2026-10-01", "2026-09-30"] {
        let temp = tempfile::tempdir().unwrap();
        let store = Store::new(temp.path().to_path_buf());
        let path = store
            .dir
            .join(format!("{}.md", date(previous).format("%Y-%m")));
        let source = format!("# {previous}\n## To do\n## Doing\n- ongoing\n## Done\n");
        fs::write(&path, &source).unwrap();
        fs::write(store.dir.join("backups"), "blocks backup creation").unwrap();
        assert!(store.new_day(date("2026-10-02")).is_err());
        assert_eq!(fs::read_to_string(path).unwrap(), source);
        assert!(
            store
                .days(Some((date("2026-10-02"), date("2026-10-02"))))
                .unwrap()
                .is_empty()
        );
    }
}
