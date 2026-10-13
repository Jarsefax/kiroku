//! Durable replacement with a backup of the previous contents.
//! Callers hold the data directory's exclusive Kiroku lock throughout save.
use anyhow::{Context, Result, bail, ensure};
use std::{
    fs::{self, File},
    io::{self, Write},
    path::{Path, PathBuf},
};
use tempfile::NamedTempFile;

const BACKUPS_TO_KEEP: usize = 5;

// Tag the real IO operations so tests can inject failures at precise stages,
// including a partial write and a failure after the log was replaced.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Stage {
    WriteLog,
    FlushLog,
    ReplaceLog,
    WriteBackup,
    FlushBackup,
    ReplaceBackup,
    FlushBackupDirectory,
    FlushBackupParent,
    FlushLogDirectory,
    PruneBackup,
    FlushPrunedDirectory,
}

trait Operations {
    fn write(&self, stage: Stage, file: &mut NamedTempFile, data: &[u8]) -> io::Result<()>;
    fn flush(&self, stage: Stage, file: &NamedTempFile) -> io::Result<()>;
    fn replace(&self, stage: Stage, file: NamedTempFile, path: &Path) -> io::Result<()>;
    fn flush_directory(&self, stage: Stage, path: &Path) -> io::Result<()>;
    fn remove(&self, stage: Stage, path: &Path) -> io::Result<()>;
}

struct Disk;
impl Operations for Disk {
    fn write(&self, _: Stage, file: &mut NamedTempFile, data: &[u8]) -> io::Result<()> {
        file.write_all(data)
    }
    fn flush(&self, _: Stage, file: &NamedTempFile) -> io::Result<()> {
        file.as_file().sync_all()
    }
    fn replace(&self, stage: Stage, file: NamedTempFile, path: &Path) -> io::Result<()> {
        // Backup names are immutable: never overwrite an existing version.
        if stage == Stage::ReplaceBackup {
            file.persist_noclobber(path).map_err(|e| e.error)?;
        } else {
            file.persist(path).map_err(|e| e.error)?;
        }
        Ok(())
    }
    fn flush_directory(&self, _: Stage, path: &Path) -> io::Result<()> {
        File::open(path)?.sync_all()
    }
    fn remove(&self, _: Stage, path: &Path) -> io::Result<()> {
        fs::remove_file(path)
    }
}

fn check_original(path: &Path, original: Option<&str>) -> Result<()> {
    match (original, fs::read_to_string(path)) {
        (Some(old), Ok(current)) if old == current => Ok(()),
        (None, Err(e)) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        (_, Err(e)) if e.kind() != io::ErrorKind::NotFound => {
            Err(e).with_context(|| format!("Cannot recheck {}", path.display()))
        }
        _ => bail!(
            "{} changed while Kiroku was editing it; review the file before retrying",
            path.display()
        ),
    }
}

fn versions(directory: &Path, name: &str) -> Result<Vec<(u64, PathBuf)>> {
    let prefix = format!("{name}.");
    let mut result = vec![];
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let filename = entry.file_name();
        let Some(filename) = filename.to_str() else {
            continue;
        };
        let Some(number) = filename
            .strip_prefix(&prefix)
            .and_then(|s| s.strip_suffix(".bak"))
        else {
            continue;
        };
        if number.is_empty() || !number.bytes().all(|b| b.is_ascii_digit()) {
            continue;
        }
        let number = number
            .parse::<u64>()
            .context("Backup version number is too large")?;
        ensure!(
            entry.file_type()?.is_file(),
            "Backup {} is not a regular file",
            entry.path().display()
        );
        result.push((number, entry.path()));
    }
    result.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
    Ok(result)
}

fn closed_month_backups(directory: &Path, parent: &Path, current: &str) -> Result<Vec<PathBuf>> {
    let mut result = vec![];
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let filename = entry.file_name();
        let Some((name, number)) = filename
            .to_str()
            .and_then(|s| s.strip_suffix(".bak"))
            .and_then(|s| s.rsplit_once('.'))
        else {
            continue;
        };
        if !name.is_ascii()
            || name.len() != 10
            || !name.ends_with(".md")
            || name >= current
            || crate::storage::parse_date(&format!("{}-01", &name[..7])).is_err()
            || number.is_empty()
            || !number.bytes().all(|b| b.is_ascii_digit())
        {
            continue;
        }
        // A missing original may need recovery. Do not follow symlinks or
        // delete anything other than ordinary backup files.
        match fs::symlink_metadata(parent.join(name)) {
            Ok(metadata) if metadata.is_file() => {}
            Ok(_) => continue,
            Err(e) if e.kind() == io::ErrorKind::NotFound => continue,
            Err(e) => return Err(e).context("Cannot check closed monthly log"),
        }
        if entry.file_type()?.is_file() {
            result.push(entry.path());
        }
    }
    Ok(result)
}

pub(crate) fn save(path: &Path, original: Option<&str>, content: &str) -> Result<()> {
    save_with(&Disk, path, original, content)
}

pub(crate) fn save_retaining_months(
    path: &Path,
    original: Option<&str>,
    content: &str,
) -> Result<()> {
    save_with_cleanup(&Disk, path, original, content, false)
}

fn save_with(
    ops: &impl Operations,
    path: &Path,
    original: Option<&str>,
    content: &str,
) -> Result<()> {
    save_with_cleanup(ops, path, original, content, true)
}

fn save_with_cleanup(
    ops: &impl Operations,
    path: &Path,
    original: Option<&str>,
    content: &str,
    close_previous_months: bool,
) -> Result<()> {
    if original == Some(content) {
        return check_original(path, original);
    }
    let parent = path.parent().context("Missing log directory")?;
    let temporary = prepare(ops, path, original, content)?;
    replace(ops, path, original, temporary)?;

    ops.flush_directory(Stage::FlushLogDirectory, parent).with_context(|| format!(
        "Update is already visible in {}, but the final directory flush failed; persistence after a crash is uncertain. Do not blindly repeat the command: inspect the file or run `krk list` first. Previous contents are in backups/ when the file already existed",
        path.display()
    ))?;
    cleanup(ops, path, close_previous_months)
}

fn prepare(
    ops: &impl Operations,
    path: &Path,
    original: Option<&str>,
    content: &str,
) -> Result<NamedTempFile> {
    let parent = path.parent().context("Missing log directory")?;
    // All failures in this block happen before replacement of the live log.
    let preparation = || -> Result<NamedTempFile> {
        check_original(path, original)?;
        let permissions = if original.is_some() {
            Some(fs::metadata(path)?.permissions())
        } else {
            None
        };
        let mut temporary =
            NamedTempFile::new_in(parent).context("Cannot create temporary log file")?;
        if let Some(permissions) = &permissions {
            temporary.as_file().set_permissions(permissions.clone())?;
        }
        ops.write(Stage::WriteLog, &mut temporary, content.as_bytes())
            .context("Cannot write updated log")?;
        ops.flush(Stage::FlushLog, &temporary)
            .context("Cannot flush updated log")?;

        if let Some(old) = original {
            let directory = parent.join("backups");
            fs::create_dir_all(&directory).context("Cannot create backup directory")?;
            let name = path
                .file_name()
                .and_then(|s| s.to_str())
                .context("Invalid log filename")?;
            let previous = versions(&directory, name)?;
            let next = previous
                .last()
                .map_or(Some(1), |(number, _)| number.checked_add(1))
                .context("Backup version numbers are exhausted")?;
            let backup_path = directory.join(format!("{name}.{next:06}.bak"));
            let mut backup =
                NamedTempFile::new_in(&directory).context("Cannot create temporary backup file")?;
            if let Some(permissions) = &permissions {
                backup.as_file().set_permissions(permissions.clone())?;
            }
            ops.write(Stage::WriteBackup, &mut backup, old.as_bytes())
                .context("Cannot write previous contents to backup")?;
            ops.flush(Stage::FlushBackup, &backup)
                .context("Cannot flush backup contents")?;
            ops.replace(Stage::ReplaceBackup, backup, &backup_path)
                .context("Cannot install backup")?;
            ops.flush_directory(Stage::FlushBackupDirectory, &directory)
                .context("Cannot flush backup directory")?;
            // Flush the parent as well, so a newly created backups/ directory
            // is durably linked before replacing any original log.
            ops.flush_directory(Stage::FlushBackupParent, parent)
                .context("Cannot flush backup directory's parent")?;
        }

        Ok(temporary)
    };
    preparation().with_context(|| {
        format!(
            "Save aborted before replacement; Kiroku did not change {}",
            path.display()
        )
    })
}

fn replace(
    ops: &impl Operations,
    path: &Path,
    original: Option<&str>,
    temporary: NamedTempFile,
) -> Result<()> {
    // Backup preparation takes time; recheck immediately before committing.
    let commit = || -> Result<()> {
        check_original(path, original)?;
        ops.replace(Stage::ReplaceLog, temporary, path)
            .context("Cannot replace log")
    };
    commit().with_context(|| {
        format!(
            "Save aborted before replacement; Kiroku did not change {}",
            path.display()
        )
    })
}

fn cleanup(ops: &impl Operations, path: &Path, close_previous_months: bool) -> Result<()> {
    let parent = path.parent().context("Missing log directory")?;
    // Only prune after a successful, flushed log replacement. A failed save
    // may retain extra backups rather than risking deletion of recovery data.
    let cleanup = || -> Result<()> {
        let directory = parent.join("backups");
        if directory.try_exists()? {
            let name = path
                .file_name()
                .and_then(|s| s.to_str())
                .context("Invalid log filename")?;
            let previous = versions(&directory, name)?;
            let remove_count = previous.len().saturating_sub(BACKUPS_TO_KEEP);
            let closed = if close_previous_months {
                closed_month_backups(&directory, parent, name)?
            } else {
                vec![]
            };
            for (_, backup) in previous.iter().take(remove_count) {
                ops.remove(Stage::PruneBackup, backup)?;
            }
            for backup in &closed {
                ops.remove(Stage::PruneBackup, backup)?;
            }
            if remove_count > 0 || !closed.is_empty() {
                ops.flush_directory(Stage::FlushPrunedDirectory, &directory)?;
            }
        }
        Ok(())
    };
    cleanup().with_context(|| format!(
        "Update was saved successfully to {}, but backup cleanup failed. Do not repeat the command; inspect backups/ (extra versions may remain)", path.display()
    ))?;
    Ok(())
}

pub(crate) struct LogUpdate<'a> {
    pub path: &'a Path,
    pub original: Option<&'a str>,
    pub content: &'a str,
}

pub(crate) fn save_move(destination: LogUpdate<'_>, source: LogUpdate<'_>) -> Result<()> {
    save_move_with(&Disk, destination, source)
}

fn save_move_with(
    ops: &impl Operations,
    destination: LogUpdate<'_>,
    source: LogUpdate<'_>,
) -> Result<()> {
    let parent = destination.path.parent().context("Missing log directory")?;
    let destination_temp = prepare(
        ops,
        destination.path,
        destination.original,
        destination.content,
    )?;
    let source_temp = prepare(ops, source.path, source.original, source.content)?;
    check_original(source.path, source.original)?;
    replace(
        ops,
        destination.path,
        destination.original,
        destination_temp,
    )?;
    let complete = || -> Result<()> {
        // The destination must be durable before the source entry is removed.
        ops.flush_directory(Stage::FlushLogDirectory, parent)
            .context("Cannot flush destination log directory")?;
        replace(ops, source.path, source.original, source_temp)?;
        ops.flush_directory(Stage::FlushLogDirectory, parent)
            .context("Cannot flush source log directory")?;
        Ok(())
    };
    complete().with_context(|| format!(
        "Cross-month move may be partially applied: the destination {} was updated, and the source {} may still contain the item. Inspect both files before retrying; a duplicate may need removing. Persistence after a failed directory flush is uncertain. Previous contents of existing files are in backups/",
        destination.path.display(), source.path.display()
    ))?;
    // Do not prune either month's backups until both logs have been flushed.
    cleanup(ops, destination.path, false)?;
    cleanup(ops, source.path, false)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::{Cell, RefCell};

    struct Fault {
        stage: Stage,
        seen: RefCell<Vec<Stage>>,
        fail_on: usize,
        calls: Cell<usize>,
    }
    impl Fault {
        fn check(&self, stage: Stage) -> io::Result<()> {
            self.seen.borrow_mut().push(stage);
            if self.stage == stage {
                self.calls.set(self.calls.get() + 1);
                if self.calls.get() == self.fail_on {
                    return Err(io::Error::other(format!("injected {stage:?} failure")));
                }
            }
            Ok(())
        }
    }
    impl Operations for Fault {
        fn write(&self, stage: Stage, file: &mut NamedTempFile, data: &[u8]) -> io::Result<()> {
            if self.stage == stage && self.calls.get() + 1 == self.fail_on {
                // Simulate a full disk after part of the file was written.
                file.write_all(&data[..data.len() / 2])?;
            }
            self.check(stage)?;
            Disk.write(stage, file, data)
        }
        fn flush(&self, stage: Stage, file: &NamedTempFile) -> io::Result<()> {
            self.check(stage)?;
            Disk.flush(stage, file)
        }
        fn replace(&self, stage: Stage, file: NamedTempFile, path: &Path) -> io::Result<()> {
            self.check(stage)?;
            Disk.replace(stage, file, path)
        }
        fn flush_directory(&self, stage: Stage, path: &Path) -> io::Result<()> {
            self.check(stage)?;
            Disk.flush_directory(stage, path)
        }
        fn remove(&self, stage: Stage, path: &Path) -> io::Result<()> {
            self.check(stage)?;
            Disk.remove(stage, path)
        }
    }

    fn fault(stage: Stage) -> Fault {
        fault_on(stage, 1)
    }

    fn fault_on(stage: Stage, fail_on: usize) -> Fault {
        Fault {
            stage,
            seen: RefCell::new(vec![]),
            fail_on,
            calls: Cell::new(0),
        }
    }

    #[test]
    fn move_preparation_failures_leave_both_logs_unchanged() {
        for stage in [
            Stage::WriteLog,
            Stage::FlushLog,
            Stage::WriteBackup,
            Stage::FlushBackup,
            Stage::ReplaceBackup,
            Stage::FlushBackupDirectory,
            Stage::FlushBackupParent,
        ] {
            for fail_on in [1, 2] {
                let temp = tempfile::tempdir().unwrap();
                let source = temp.path().join("2026-09.md");
                let destination = temp.path().join("2026-10.md");
                fs::write(&source, "source item\n").unwrap();
                fs::write(&destination, "existing destination\n").unwrap();
                let ops = fault_on(stage, fail_on);
                let error = save_move_with(
                    &ops,
                    LogUpdate {
                        path: &destination,
                        original: Some("existing destination\n"),
                        content: "existing destination\nsource item\n",
                    },
                    LogUpdate {
                        path: &source,
                        original: Some("source item\n"),
                        content: "",
                    },
                )
                .unwrap_err();
                assert!(format!("{error:#}").contains("Save aborted before replacement"));
                assert_eq!(
                    fs::read_to_string(&source).unwrap(),
                    "source item\n",
                    "{stage:?} #{fail_on}"
                );
                assert_eq!(
                    fs::read_to_string(&destination).unwrap(),
                    "existing destination\n",
                    "{stage:?} #{fail_on}"
                );
                assert!(!ops.seen.borrow().contains(&Stage::ReplaceLog));
            }
        }
    }

    #[test]
    fn interrupted_moves_keep_the_item_and_report_partial_application() {
        for (stage, fail_on, destination_saved, source_saved) in [
            (Stage::ReplaceLog, 1, false, false),
            (Stage::ReplaceLog, 2, true, false),
            (Stage::FlushLogDirectory, 1, true, false),
            (Stage::FlushLogDirectory, 2, true, true),
        ] {
            let temp = tempfile::tempdir().unwrap();
            let source = temp.path().join("2026-09.md");
            let destination = temp.path().join("2026-10.md");
            fs::write(&source, "source item\n").unwrap();
            let ops = fault_on(stage, fail_on);
            let error = save_move_with(
                &ops,
                LogUpdate {
                    path: &destination,
                    original: None,
                    content: "source item\n",
                },
                LogUpdate {
                    path: &source,
                    original: Some("source item\n"),
                    content: "",
                },
            )
            .unwrap_err();
            let error = format!("{error:#}");
            assert_eq!(
                error.contains("Cross-month move may be partially applied"),
                destination_saved,
                "{error}"
            );
            assert_eq!(destination.exists(), destination_saved);
            if destination_saved {
                assert_eq!(fs::read_to_string(&destination).unwrap(), "source item\n");
                assert!(error.contains("Inspect both files before retrying"));
            }
            assert_eq!(
                fs::read_to_string(&source).unwrap(),
                if source_saved { "" } else { "source item\n" }
            );
            assert_eq!(
                fs::read_to_string(temp.path().join("backups/2026-09.md.000001.bak")).unwrap(),
                "source item\n"
            );
        }
    }

    #[test]
    fn move_cleanup_failure_happens_after_both_files_are_saved() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("2026-09.md");
        let destination = temp.path().join("2026-10.md");
        fs::write(&source, "source item\n").unwrap();
        let backups = temp.path().join("backups");
        fs::create_dir(&backups).unwrap();
        for n in 1..=5 {
            fs::write(backups.join(format!("2026-09.md.{n:06}.bak")), "older").unwrap();
        }
        let error = save_move_with(
            &fault(Stage::PruneBackup),
            LogUpdate {
                path: &destination,
                original: None,
                content: "source item\n",
            },
            LogUpdate {
                path: &source,
                original: Some("source item\n"),
                content: "",
            },
        )
        .unwrap_err();
        assert!(format!("{error:#}").contains("Update was saved successfully"));
        assert_eq!(fs::read_to_string(source).unwrap(), "");
        assert_eq!(fs::read_to_string(destination).unwrap(), "source item\n");
        assert_eq!(
            fs::read_to_string(backups.join("2026-09.md.000006.bak")).unwrap(),
            "source item\n"
        );
    }

    #[test]
    fn pre_replacement_failures_leave_original_and_existing_backups_intact() {
        for stage in [
            Stage::WriteLog,
            Stage::FlushLog,
            Stage::WriteBackup,
            Stage::FlushBackup,
            Stage::ReplaceBackup,
            Stage::FlushBackupDirectory,
            Stage::FlushBackupParent,
            Stage::ReplaceLog,
        ] {
            let temp = tempfile::tempdir().unwrap();
            let path = temp.path().join("2026-10.md");
            let directory = temp.path().join("backups");
            fs::create_dir(&directory).unwrap();
            fs::write(&path, "original contents\n").unwrap();
            for n in 1..=5 {
                fs::write(
                    directory.join(format!("2026-10.md.{n:06}.bak")),
                    format!("old version {n}"),
                )
                .unwrap();
            }
            let ops = fault(stage);
            let error = save_with(
                &ops,
                &path,
                Some("original contents\n"),
                "updated contents\n",
            )
            .unwrap_err();
            assert!(
                format!("{error:#}").contains("Save aborted before replacement"),
                "{error:#}"
            );
            assert_eq!(
                fs::read_to_string(&path).unwrap(),
                "original contents\n",
                "{stage:?}"
            );
            for n in 1..=5 {
                assert_eq!(
                    fs::read_to_string(directory.join(format!("2026-10.md.{n:06}.bak"))).unwrap(),
                    format!("old version {n}")
                );
            }
            assert!(ops.seen.borrow().contains(&stage));
            // Temporary files are removed even when a write only partially succeeds.
            assert_eq!(fs::read_dir(temp.path()).unwrap().count(), 2);
            assert!(
                fs::read_dir(directory).unwrap().all(|e| e
                    .unwrap()
                    .file_name()
                    .to_string_lossy()
                    .ends_with(".bak"))
            );
        }
    }

    #[test]
    fn final_directory_flush_failure_reports_already_applied_update_and_keeps_backup() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("2026-10.md");
        fs::write(&path, "old\r\n").unwrap();
        let error = save_with(
            &fault(Stage::FlushLogDirectory),
            &path,
            Some("old\r\n"),
            "new\r\n",
        )
        .unwrap_err();
        let message = format!("{error:#}");
        assert!(message.contains("already visible"));
        assert!(message.contains("Do not blindly repeat"));
        assert!(!message.contains("Save aborted"));
        assert_eq!(fs::read_to_string(&path).unwrap(), "new\r\n");
        assert_eq!(
            fs::read(temp.path().join("backups/2026-10.md.000001.bak")).unwrap(),
            b"old\r\n"
        );
    }

    #[test]
    fn new_file_final_flush_failure_also_reports_applied_update() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("2026-10.md");
        let error =
            save_with(&fault(Stage::FlushLogDirectory), &path, None, "new file\n").unwrap_err();
        assert!(format!("{error:#}").contains("already visible"));
        assert_eq!(fs::read_to_string(&path).unwrap(), "new file\n");
        assert!(!temp.path().join("backups").exists());
    }

    #[test]
    fn successful_saves_keep_exact_previous_contents_and_five_versions_for_active_month() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("2026-10.md");
        save(&path, None, "version 0\r\n<!-- note -->\r\n").unwrap();
        assert!(!temp.path().join("backups").exists());
        let mut previous = "version 0\r\n<!-- note -->\r\n".to_owned();
        for n in 1..=8 {
            let next = format!("version {n}\r\n<!-- note -->\r\n");
            save(&path, Some(&previous), &next).unwrap();
            previous = next;
        }
        let directory = temp.path().join("backups");
        let files = versions(&directory, "2026-10.md").unwrap();
        assert_eq!(
            files.iter().map(|(n, _)| *n).collect::<Vec<_>>(),
            [4, 5, 6, 7, 8]
        );
        for (n, file) in files {
            assert_eq!(
                fs::read_to_string(file).unwrap(),
                format!("version {}\r\n<!-- note -->\r\n", n - 1)
            );
        }
        let other = temp.path().join("2026-11.md");
        save(&other, None, "other old").unwrap();
        assert!(versions(&directory, "2026-10.md").unwrap().is_empty());
        assert_eq!(fs::read_to_string(&path).unwrap(), previous);
        save(&other, Some("other old"), "other new").unwrap();
        assert!(versions(&directory, "2026-10.md").unwrap().is_empty());
        assert_eq!(versions(&directory, "2026-11.md").unwrap().len(), 1);
    }

    #[test]
    fn failed_month_rollover_keeps_closed_month_backups() {
        for stage in [
            Stage::WriteLog,
            Stage::FlushLog,
            Stage::ReplaceLog,
            Stage::FlushLogDirectory,
        ] {
            let temp = tempfile::tempdir().unwrap();
            let old = temp.path().join("2026-10.md");
            save(&old, None, "old version").unwrap();
            save(&old, Some("old version"), "closed month").unwrap();
            let next = temp.path().join("2026-11.md");
            let ops = fault(stage);
            assert!(save_with(&ops, &next, None, "new month").is_err());
            assert!(ops.seen.borrow().contains(&stage));
            assert!(!ops.seen.borrow().contains(&Stage::PruneBackup));
            assert_eq!(fs::read_to_string(&old).unwrap(), "closed month");
            assert_eq!(
                fs::read_to_string(temp.path().join("backups/2026-10.md.000001.bak")).unwrap(),
                "old version"
            );
        }
    }

    #[test]
    fn failed_update_to_older_month_recreates_backup_after_rollover_cleanup() {
        for stage in [Stage::ReplaceLog, Stage::FlushLogDirectory] {
            let temp = tempfile::tempdir().unwrap();
            let old = temp.path().join("2026-10.md");
            save(&old, None, "first version").unwrap();
            save(&old, Some("first version"), "closed month\r\n").unwrap();
            save(&temp.path().join("2026-11.md"), None, "new month").unwrap();
            let directory = temp.path().join("backups");
            assert!(versions(&directory, "2026-10.md").unwrap().is_empty());

            let ops = fault(stage);
            assert!(save_with(&ops, &old, Some("closed month\r\n"), "edited month").is_err());
            assert!(ops.seen.borrow().contains(&stage));
            let seen = ops.seen.borrow();
            assert!(
                seen.iter()
                    .position(|s| *s == Stage::FlushBackupDirectory)
                    .unwrap()
                    < seen.iter().position(|s| *s == Stage::ReplaceLog).unwrap()
            );
            assert_eq!(
                fs::read(directory.join("2026-10.md.000001.bak")).unwrap(),
                b"closed month\r\n"
            );
            assert_eq!(
                fs::read_to_string(&old).unwrap(),
                if stage == Stage::ReplaceLog {
                    "closed month\r\n"
                } else {
                    "edited month"
                }
            );
        }
    }

    #[test]
    fn closed_month_cleanup_preserves_recovery_and_unrelated_files() {
        let temp = tempfile::tempdir().unwrap();
        let directory = temp.path().join("backups");
        fs::create_dir(&directory).unwrap();
        for name in ["2025-10.md", "2025-12.md", "2026-02.md", "2025-13.md"] {
            fs::write(temp.path().join(name), "original").unwrap();
        }
        let retained = [
            "2025-11.md.000001.bak", // Missing original.
            "2026-02.md.000001.bak", // Later month.
            "2025-13.md.000001.bak", // Invalid month.
            "2025-12.md.notes.bak",  // Not a version.
            "2025-12.md..bak",       // Empty version.
            "notes.000001.bak",      // Unrelated file.
        ];
        for name in retained {
            fs::write(directory.join(name), "keep").unwrap();
        }
        for name in ["2025-10.md.000001.bak", "2025-12.md.000001.bak"] {
            fs::write(directory.join(name), "old backup").unwrap();
        }
        // A directory with a backup-like name must not be removed.
        fs::create_dir(directory.join("2025-12.md.000002.bak")).unwrap();
        save(&temp.path().join("2026-01.md"), None, "new year").unwrap();
        for name in retained {
            assert_eq!(fs::read_to_string(directory.join(name)).unwrap(), "keep");
        }
        for name in ["2025-10.md.000001.bak", "2025-12.md.000001.bak"] {
            assert!(!directory.join(name).exists());
        }
        assert!(directory.join("2025-12.md.000002.bak").is_dir());
        for name in ["2025-10.md", "2025-12.md"] {
            assert_eq!(
                fs::read_to_string(temp.path().join(name)).unwrap(),
                "original"
            );
        }
    }

    #[test]
    fn rollover_cleanup_failure_reports_saved_update_and_next_write_retries_cleanup() {
        for stage in [Stage::PruneBackup, Stage::FlushPrunedDirectory] {
            let temp = tempfile::tempdir().unwrap();
            let old = temp.path().join("2026-10.md");
            save(&old, None, "old version").unwrap();
            save(&old, Some("old version"), "closed month").unwrap();
            let next = temp.path().join("2026-11.md");
            let ops = fault(stage);
            let error = save_with(&ops, &next, None, "new month").unwrap_err();
            assert!(format!("{error:#}").contains("Update was saved successfully"));
            assert!(format!("{error:#}").contains("Do not repeat the command"));
            assert!(ops.seen.borrow().contains(&stage));
            assert_eq!(fs::read_to_string(&next).unwrap(), "new month");
            assert_eq!(fs::read_to_string(&old).unwrap(), "closed month");
            save(&next, Some("new month"), "updated month").unwrap();
            let directory = temp.path().join("backups");
            assert!(versions(&directory, "2026-10.md").unwrap().is_empty());
            assert_eq!(
                fs::read_to_string(directory.join("2026-11.md.000001.bak")).unwrap(),
                "new month"
            );
        }
    }

    #[test]
    fn cleanup_failure_reports_saved_update_and_does_not_erase_newest_backup() {
        for stage in [Stage::PruneBackup, Stage::FlushPrunedDirectory] {
            let temp = tempfile::tempdir().unwrap();
            let path = temp.path().join("2026-10.md");
            let directory = temp.path().join("backups");
            fs::create_dir(&directory).unwrap();
            fs::write(&path, "old").unwrap();
            for n in 1..=5 {
                fs::write(directory.join(format!("2026-10.md.{n:06}.bak")), "older").unwrap();
            }
            let error = save_with(&fault(stage), &path, Some("old"), "new").unwrap_err();
            assert!(format!("{error:#}").contains("Update was saved successfully"));
            assert_eq!(fs::read_to_string(path).unwrap(), "new");
            assert_eq!(
                fs::read_to_string(directory.join("2026-10.md.000006.bak")).unwrap(),
                "old"
            );
        }
    }

    #[test]
    fn unavailable_backup_directory_aborts_without_touching_log() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("2026-10.md");
        fs::write(&path, "old").unwrap();
        fs::write(temp.path().join("backups"), "not a directory").unwrap();
        assert!(save(&path, Some("old"), "new").is_err());
        assert_eq!(fs::read_to_string(path).unwrap(), "old");
    }

    #[test]
    fn failed_creation_does_not_leave_a_partial_live_file() {
        for stage in [Stage::WriteLog, Stage::FlushLog, Stage::ReplaceLog] {
            let temp = tempfile::tempdir().unwrap();
            let path = temp.path().join("2026-10.md");
            let error = save_with(&fault(stage), &path, None, "new file contents\n").unwrap_err();
            assert!(format!("{error:#}").contains("Save aborted before replacement"));
            assert!(!path.exists());
            assert_eq!(fs::read_dir(temp.path()).unwrap().count(), 0);
        }
    }

    #[test]
    fn unchanged_save_does_not_create_backups() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("2026-10.md");
        fs::write(&path, "unchanged").unwrap();
        save(&path, Some("unchanged"), "unchanged").unwrap();
        assert!(!temp.path().join("backups").exists());
        assert_eq!(fs::read_to_string(&path).unwrap(), "unchanged");
    }

    #[test]
    fn externally_changed_log_is_not_overwritten_or_backed_up_as_stale_data() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("2026-10.md");
        fs::write(&path, "external edit").unwrap();
        assert!(save(&path, Some("old"), "new").is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), "external edit");
        assert!(!temp.path().join("backups").exists());
        assert!(save(&path, None, "new file").is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), "external edit");
    }

    #[cfg(unix)]
    #[test]
    fn original_permissions_are_preserved_on_log_and_backup() {
        use std::os::unix::fs::PermissionsExt;
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("2026-10.md");
        fs::write(&path, "old").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o640)).unwrap();
        save(&path, Some("old"), "new").unwrap();
        for path in [path, temp.path().join("backups/2026-10.md.000001.bak")] {
            assert_eq!(
                fs::metadata(path).unwrap().permissions().mode() & 0o777,
                0o640
            );
        }
    }
}
