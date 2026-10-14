use kiroku::config;
use std::fs;

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
