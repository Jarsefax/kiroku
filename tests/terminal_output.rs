#![cfg(target_os = "linux")]

use rustix::pty::{OpenptFlags, grantpt, openpt, ptsname, unlockpt};
use std::{
    ffi::OsStr,
    fs::{File, OpenOptions},
    io::Read,
    os::unix::ffi::OsStrExt,
    path::Path,
    process::{Command, Output, Stdio},
};

fn redirected(directory: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_krk"))
        .arg("--data-dir")
        .arg(directory)
        .args(args)
        .env("XDG_CONFIG_HOME", directory.join(".test-config"))
        .output()
        .unwrap()
}

fn terminal(directory: &Path, args: &[&str], term: &str, no_color: bool) -> Output {
    let master = openpt(OpenptFlags::RDWR | OpenptFlags::NOCTTY | OpenptFlags::CLOEXEC).unwrap();
    grantpt(&master).unwrap();
    unlockpt(&master).unwrap();
    let name = ptsname(&master, Vec::new()).unwrap();
    let slave = OpenOptions::new()
        .read(true)
        .write(true)
        .open(Path::new(OsStr::from_bytes(name.as_bytes())))
        .unwrap();
    let mut command = Command::new(env!("CARGO_BIN_EXE_krk"));
    command
        .arg("--data-dir")
        .arg(directory)
        .args(args)
        .env("XDG_CONFIG_HOME", directory.join(".test-config"))
        .env("TERM", term)
        .env_remove("NO_COLOR")
        .stdin(Stdio::null())
        .stdout(slave)
        .stderr(Stdio::piped());
    if no_color {
        command.env("NO_COLOR", "1");
    }
    let child = command.spawn().unwrap();
    // Close the parent's slave so reads finish when the child exits.
    drop(command);
    let mut master = File::from(master);
    let mut stdout = Vec::new();
    let mut chunk = [0; 4096];
    loop {
        match master.read(&mut chunk) {
            Ok(0) => break,
            Ok(count) => stdout.extend_from_slice(&chunk[..count]),
            // Linux PTY masters report EIO after the last slave closes.
            Err(error) if error.raw_os_error() == Some(rustix::io::Errno::IO.raw_os_error()) => {
                break;
            }
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(error) => panic!("Cannot read terminal output: {error}"),
        }
    }
    let mut output = child.wait_with_output().unwrap();
    output.stdout = String::from_utf8(stdout)
        .unwrap()
        .replace("\r\n", "\n")
        .into_bytes();
    output
}

fn success(output: Output) -> String {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

fn seed(directory: &Path) {
    success(redirected(directory, &["todo", "first task"]));
    success(redirected(directory, &["todo", "second task"]));
}

fn without_separator(text: &str, command: &str) -> String {
    assert!(
        text.starts_with(&format!("\n── krk {command} ")),
        "{text:?}"
    );
    assert_eq!(text.matches("── krk ").count(), 1, "{text:?}");
    assert!(
        !text.contains('\x1b'),
        "Plain output must have no terminal controls"
    );
    text.lines().skip(2).collect::<Vec<_>>().join("\n") + "\n"
}

#[test]
fn long_terminal_listings_have_one_heading_and_preserve_the_body() {
    let temp = tempfile::tempdir().unwrap();
    seed(temp.path());
    for (args, command) in [
        (&["list", "--color", "never"][..], "list"),
        (&["l", "--color", "never"][..], "list"),
        (&["search", "task", "--color", "never"][..], "search"),
        (
            &["list", "--status", "todo", "--color", "never"][..],
            "list",
        ),
    ] {
        let body = success(redirected(temp.path(), args));
        let decorated = success(terminal(temp.path(), args, "xterm-256color", false));
        assert_eq!(without_separator(&decorated, command), body);
    }
}

#[test]
fn updates_and_chunked_output_start_with_one_separator() {
    let temp = tempfile::tempdir().unwrap();
    seed(temp.path());
    for args in [
        &["new-day"][..],
        &["todo", "third task"],
        &["doing", "1"],
        &["done", "1"],
        &["repeat", "1"],
        &["edit", "1", "renamed task"],
        &["move", "1", "down"],
        &["remove", "1"],
        &["todo", "dated task", "--date", "2000-01-01"],
        &["search", "task", "--repeat", "1"],
        &["list", "--remove", "1"],
    ] {
        let mut plain_args = args.to_vec();
        plain_args.extend(["--color", "never"]);
        let text = success(terminal(temp.path(), &plain_args, "xterm-256color", false));
        let body = without_separator(&text, args[0]);
        assert!(!body.trim().is_empty());
        if args.contains(&"--repeat") {
            assert!(body.starts_with("Repeated result "));
        }
        if args.contains(&"--remove") {
            assert!(body.starts_with("Removed result "));
        }
    }
}

#[test]
fn short_output_errors_and_informational_commands_have_no_separator() {
    let temp = tempfile::tempdir().unwrap();
    for args in [
        &["list"][..],
        &["search", "missing"],
        &["path"],
        &["list", "--help"],
        &["--help"],
        &["--version"],
        &["done", "1"],
        &["list", "invalid-period"],
    ] {
        let mut plain_args = args.to_vec();
        plain_args.extend(["--color", "never"]);
        let expected = redirected(temp.path(), &plain_args);
        // Clap styles terminal help independently of the command's --color option.
        let actual = terminal(temp.path(), &plain_args, "xterm-256color", true);
        assert_eq!(actual.status.code(), expected.status.code());
        assert_eq!(actual.stdout, expected.stdout);
        assert_eq!(actual.stderr, expected.stderr);
    }
    seed(temp.path());
    let args = ["search", "missing", "--color", "never"];
    assert_eq!(
        success(terminal(temp.path(), &args, "xterm-256color", false)),
        success(redirected(temp.path(), &args))
    );
    let rejected = terminal(temp.path(), &["list", "--clear"], "xterm-256color", false);
    assert!(!rejected.status.success());
    assert!(rejected.stdout.is_empty());
}

#[test]
fn separator_opt_out_works_before_and_after_the_command() {
    let temp = tempfile::tempdir().unwrap();
    seed(temp.path());
    let body = success(redirected(temp.path(), &["list", "--color", "never"]));
    for args in [
        &["--no-separator", "list", "--color", "never"][..],
        &["l", "--color", "never", "--no-separator"],
    ] {
        assert_eq!(
            success(terminal(temp.path(), args, "xterm-256color", false)),
            body
        );
    }
}

#[test]
fn separator_respects_color_and_remains_visible_without_ansi() {
    let temp = tempfile::tempdir().unwrap();
    seed(temp.path());
    let styled = success(terminal(temp.path(), &["list"], "xterm-256color", false));
    assert!(styled.starts_with("\n\x1b[2m── krk list "));
    for (args, term, no_color) in [
        (&["list", "--color", "never"][..], "xterm-256color", false),
        (&["list"][..], "xterm-256color", true),
        (&["list"][..], "dumb", false),
    ] {
        let text = success(terminal(temp.path(), args, term, no_color));
        without_separator(&text, "list");
    }
    let forced = success(terminal(
        temp.path(),
        &["list", "--color", "always"],
        "dumb",
        true,
    ));
    assert!(forced.starts_with("\n\x1b[2m── krk list "));
}
