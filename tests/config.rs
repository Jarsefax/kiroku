use kiroku::config;
use std::fs;

#[test]
fn initialization_creates_a_valid_default_config_with_inactive_examples() {
    let temp = tempfile::tempdir().unwrap();
    let config_dir = temp.path().join("nested/config");
    let (path, created) = config::init_with(&config_dir).unwrap();
    assert!(created);
    assert_eq!(path, config_dir.join("kiroku/config.toml"));
    assert_eq!(
        config::resolve_with(None, temp.path(), &config_dir).unwrap(),
        temp.path().join("Documents/kiroku")
    );
    assert!(
        config::recurring_todos_with(&config_dir)
            .unwrap()
            .is_empty()
    );
    let text = fs::read_to_string(&path).unwrap();
    assert!(text.contains("# [[recurring_todos]]"));
    assert!(text.contains("--skip-recurring"));
    assert!(!temp.path().join("Documents/kiroku").exists());
}

#[test]
fn initialization_preserves_existing_configs_even_when_empty_or_invalid() {
    let temp = tempfile::tempdir().unwrap();
    let config_dir = temp.path().join("config");
    let (path, _) = config::init_with(&config_dir).unwrap();
    for existing in [
        "",
        "this is not TOML\n",
        "data_dir = '~/custom logs'\n# 日本語\n",
    ] {
        fs::write(&path, existing).unwrap();
        let (reported, created) = config::init_with(&config_dir).unwrap();
        assert!(!created);
        assert_eq!(reported, path);
        assert_eq!(fs::read_to_string(&path).unwrap(), existing);
    }
}

#[test]
fn concurrent_initialization_creates_one_complete_config_and_cleans_up_temporary_files() {
    let temp = tempfile::tempdir().unwrap();
    let config_dir = temp.path().join("config");
    let threads: Vec<_> = (0..8)
        .map(|_| {
            let config_dir = config_dir.clone();
            std::thread::spawn(move || config::init_with(&config_dir).unwrap().1)
        })
        .collect();
    assert_eq!(
        threads
            .into_iter()
            .filter_map(|thread| thread.join().unwrap().then_some(()))
            .count(),
        1
    );
    assert!(
        config::recurring_todos_with(&config_dir)
            .unwrap()
            .is_empty()
    );
    assert_eq!(fs::read_dir(config_dir.join("kiroku")).unwrap().count(), 1);
}

#[test]
fn initialization_reports_blocked_paths_without_replacing_anything() {
    let temp = tempfile::tempdir().unwrap();
    let blocked = temp.path().join("blocked");
    fs::write(&blocked, "keep me").unwrap();
    assert!(config::init_with(&blocked).is_err());
    assert_eq!(fs::read_to_string(blocked).unwrap(), "keep me");
    let config_dir = temp.path().join("config");
    let path = config_dir.join("kiroku/config.toml");
    fs::create_dir_all(&path).unwrap();
    assert!(
        config::init_with(&config_dir)
            .unwrap_err()
            .to_string()
            .contains("not a configuration file")
    );
    assert!(path.is_dir());
}

#[cfg(unix)]
#[test]
fn initialization_preserves_symlinks_without_touching_their_targets() {
    use std::os::unix::fs::symlink;
    for target_exists in [false, true] {
        let temp = tempfile::tempdir().unwrap();
        let config_dir = temp.path().join("config");
        fs::create_dir_all(config_dir.join("kiroku")).unwrap();
        let path = config_dir.join("kiroku/config.toml");
        let target = temp.path().join("target.toml");
        if target_exists {
            fs::write(&target, "existing settings\n").unwrap();
        }
        symlink(&target, &path).unwrap();
        assert!(!config::init_with(&config_dir).unwrap().1);
        assert_eq!(fs::read_link(path).unwrap(), target);
        if target_exists {
            assert_eq!(fs::read_to_string(target).unwrap(), "existing settings\n");
        } else {
            assert!(!target.exists());
        }
    }
}

#[test]
fn recurrence_is_optional_and_keeps_data_directory_resolution() {
    let temp = tempfile::tempdir().unwrap();
    let config_dir = temp.path().join("config");
    assert!(
        config::recurring_todos_with(&config_dir)
            .unwrap()
            .is_empty()
    );
    fs::create_dir_all(config_dir.join("kiroku")).unwrap();
    let path = config_dir.join("kiroku/config.toml");
    fs::write(&path, "data_dir = '~/my logs'\n").unwrap();
    assert!(
        config::recurring_todos_with(&config_dir)
            .unwrap()
            .is_empty()
    );
    fs::write(
        &path,
        r#"
data_dir = "~/my logs"
[[recurring_todos]]
title = "foo"
days = ["monday", "friday"]
"#,
    )
    .unwrap();
    assert_eq!(config::recurring_todos_with(&config_dir).unwrap().len(), 1);
    assert_eq!(
        config::resolve_with(None, temp.path(), &config_dir).unwrap(),
        temp.path().join("my logs")
    );
}

#[test]
fn invalid_recurrence_reports_the_config_path_and_does_not_create_logs() {
    let temp = tempfile::tempdir().unwrap();
    let config_dir = temp.path().join("config");
    fs::create_dir_all(config_dir.join("kiroku")).unwrap();
    let path = config_dir.join("kiroku/config.toml");
    for entry in [
        "title = 'foo'\ndays = ['funday']",
        "title = 'foo'\ndays = []",
        "title = ''\ndays = ['monday']",
        "title = '  '\ndays = ['monday']",
        "title = \"two\\nlines\"\ndays = ['monday']",
        "title = \"tab\\tcharacter\"\ndays = ['monday']",
        "title = 'foo'",
        "days = ['monday']",
        "title = 123\ndays = ['monday']",
        "title = 'foo'\ndays = 'monday'",
        "title = 'foo'\ndays = ['monday']\nunknown = true",
    ] {
        fs::write(&path, format!("[[recurring_todos]]\n{entry}\n")).unwrap();
        let error = config::recurring_todos_with(&config_dir).unwrap_err();
        assert!(
            format!("{error:#}").contains(&path.display().to_string()),
            "{error:#}"
        );
        assert!(!temp.path().join("Documents/kiroku").exists());
    }
}
