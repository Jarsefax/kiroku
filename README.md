# Kiroku

A fast terminal work log. The command is **`krk`**. Log short titles in
**To do**, **Doing**, and **Done**, grouped by date in editable monthly
Markdown files. No database and no timestamps on items.

## Getting started

```sh
krk new-day
krk todo "weekly meeting"
krk todo "monthly op meeting - september"
krk list
krk doing 1
krk list
krk done 2
krk
```

Item numbers can change after moves or removals. Every change prints the
updated active day; use the numbers from that output for the next command.
Numbers are unique across all three sections of the active day.

Run `krk --help` or `krk COMMAND --help` for command help.

## Commands

| Command | Behavior |
| --- | --- |
| `krk` | Open the keyboard-driven TUI |
| `krk new-day` | Start today explicitly; leave an existing today unchanged |
| `krk todo "title"` | Add a todo to the active day |
| `krk list` | Show today; if empty, also show the previous recorded day |
| `krk search "text" [PERIOD]` | Search titles across all recorded days, or a selected period |
| `krk list [PERIOD] --search "text"` | List matching titles; searches all history without PERIOD |
| `krk list [PERIOD] --status STATUS` | List items with the selected status; searches all history without PERIOD |
| `krk doing NUMBER` | Move an active-day item into Doing |
| `krk doing "title"` | Add an item directly to Doing |
| `krk done NUMBER` | Move an active-day item into Done |
| `krk done "title"` | Add an item directly to Done |
| `krk todo NUMBER` | Move an active-day item back into To do |
| `krk repeat NUMBER` | Copy an active-day item into To do, keeping the original |
| `krk repeat --from YYYY-MM-DD LETTER` | Copy a historical item into the active day's To do |
| `krk edit NUMBER "title"` | Rename an active-day item |
| `krk remove NUMBER` | Remove an active-day item immediately |
| `krk path` | Print the resolved log directory |

All status commands can move an item from any section. Titles must be
nonempty, single-line text; Unicode and duplicate titles are supported.
`todo`, `doing`, and `done` treat an argument containing only ASCII digits as an item number,
even when quoted; all other text is a new title. Invalid item numbers produce
an error. Quote titles containing spaces.

### Reusing a title

Repeating an item creates a new entry with its exact title and leaves the
original entry and status intact. To reopen an active-day Done item instead,
use `krk doing NUMBER` or `krk todo NUMBER`.

```sh
krk repeat 3
krk list 2026-10-02
krk repeat --from 2026-10-02 B
krk repeat --from 2026-10-02 B --status doing
```

Without `--from`, use a current active-day number. With `--from`, select an
earlier recorded date and use its uppercase letter from `krk list YYYY-MM-DD`.
Letters are scoped to that single date for repeating. Search and filtered
results use the same date-scoped letters, so you can copy a result directly.
Labels from unfiltered multi-day listings can differ. Labels past Z work as
AA, AB, etc. The source may be in any section.
`--status todo|doing|done` selects the new entry's status; the
default is To do. Copies go to the last explicitly started day, including
when it is earlier than today. Repeating never starts a day automatically;
run `krk new-day` first if there are no recorded days. Each repeat creates
another entry, even if the active day already contains that title.

### Days and rollover

Opening Kiroku, listing items, and crossing midnight do **not** start a day.
Until you run `krk new-day`, additions and changes apply to the last recorded
day, even after a weekend or across a month boundary. Dates use your local
timezone. The first day starts empty.

Starting a new day copies the previous recorded day's To do and Doing items
into the new day's To do section. Done items stay behind. Earlier records
remain intact; skipped days are not created. Starting an already recorded
today does not copy items again. Starting a date supplied on the command line
is not supported in this version.

If today has not been started, `krk list` says so and shows the last recorded
day. If today exists but is empty, it shows both today and the most recent
earlier record. Output always labels dates and the active day. Active-day
items have numbers; historical items have letters (`A`…`Z`, `AA`, etc.).
Historical letters cannot edit historical entries. `repeat --from` can copy
one using its letter from a single-date listing or search/filtered results.

### Listing dates

```sh
krk list today
krk list yesterday
krk list week
krk list month
krk list 2026-10-02
krk list 2026-09-28..2026-10-07
```

Week means the current calendar week, Monday–Sunday. Month means the
current calendar month. Ranges include both endpoints. Explicit selections
show only the requested period, with recorded days in chronological order;
they do not fall back to another day or create missing records.

### Searching and filtering

```sh
krk search "weekly meeting"
krk search "meeting" --status done
krk search "meeting" month
krk list --search "meeting" --status done
krk list week --status doing
krk list 2026-09-28..2026-10-07 --search "meeting"
```

Search matches text anywhere in a title, ignoring case. `search` and
`list --search` are equivalent. Text and status filters can be combined;
both must match. With no period, a search or filtered listing searches all
recorded days. A supplied period limits results to those dates. An empty
search matches all titles, and `--status` works without a text filter.
Plain `krk list` retains its usual today/previous-day behavior.

Results show matching items grouped by date and status, omitting days and
sections without matches. Entries keep their original active-day numbers
or date-scoped historical letters, even if that leaves gaps. Repeated titles
remain separate occurrences. To copy a historical result into today:

```sh
krk new-day
krk search "weekly meeting" --status done
krk repeat --from 2026-10-02 B
```

Use the date and letter from your results. Start today before searching so
the displayed references reflect the active day you intend to copy into.
Searching itself is read-only: it never starts a day or changes log files.

## Keyboard navigation

| Key | Action |
| --- | --- |
| ↑/↓ or `j`/`k` | Select an item |
| ←/→ or `h`/`l` | Browse earlier/later recorded days |
| Home/End or `g`/`G` | Select the first/last item |
| `a` | Add a todo |
| `c` | Copy the selected item from any day into the active day's To do |
| `/` | Search titles across all recorded days |
| `s`, `d`, `t` | Move to Doing, Done, or To do |
| `e` | Edit the selected title |
| `x` | Remove the selected item, with a `y`/`n` confirmation |
| `n` | Start today explicitly |
| `r` | Reload manually edited files |
| `?` | Show help |
| `q` or Ctrl-C | Quit |

Copying with `c` preserves the source, jumps to the active day, and selects
the new entry. Press `s` to start working on it immediately.

Press `/`, enter search text, and press Enter to list matches across all
days, newest day first. An empty query lists all items. Results show the
source date, original number/letter, status, and title. Navigate with
Up/Down or `j`/`k`; Home/End or `g`/`G` selects the first/last match.
Press `f` to cycle the status filter through all, Done, To do, and Doing.
Press `/` again to refine the query while retaining the status filter.
Enter opens the selected item's source day, `c` copies it into active To do,
and Esc leaves the results. To copy into today, press `n` first; this starts
today while retaining the selected search result. `r` refreshes results
after external edits. Opening an active-day result also lets you use the
usual editing and status shortcuts; historical entries remain read-only.

In a title prompt, type or paste a title, use Backspace to delete a character
or Ctrl-U to clear, Enter to save, and Esc to cancel. The add prompt shows up
to five matching previous titles, most recently used first. Each exact title
appears once, with its most recent completion date (or last use if never
completed). Matching ignores case and searches anywhere in the title. Use
Up/Down or Tab/Shift-Tab to select a suggestion, then Enter to reuse its exact
text. With no suggestion selected, Enter saves the text you typed. Typing or
deleting clears the suggestion selection. The app edits only the active day;
historical days can be edited directly in their Markdown files. It redraws
on input and resize rather than running an idle animation loop.

## Files and configuration

Default logs live in **`~/Document/kiroku/`**, with singular `Document`.
Each month has a file such as `2026-10.md`:

```markdown
# 2026-10-06

## To do
- monthly op meeting - september

## Doing
- weekly meeting

## Done
- update documentation
```

Append another date heading and the same three sections for each recorded
day. Dates must be unique, chronological, and belong to the filename's month.
Keep section names and their order as shown. Items use `- title`.
Blank lines and standalone HTML comments (`<!-- notes -->`, including
multiline comments) are supported and preserved. Other prose or Markdown
structures produce a parse error rather than being silently rewritten.

Kiroku preserves untouched lines and earlier day records. It saves through
an atomic replacement and serializes its writes using `.kiroku.lock` in the
data directory. Temporary files and the lock are not part of the log format.
Reload the TUI with `r` after manual edits. A stale TUI selection is rejected
before it can change a different item. Avoid saving from an external editor
at exactly the same time as an app write; external editors do not use the
app's lock.

### Backups and failed saves

Before replacing an existing monthly file, Kiroku saves its exact previous
contents in `backups/` inside the data directory, for example:

```text
kiroku/
  2026-10.md
  backups/
    2026-10.md.000001.bak
    2026-10.md.000002.bak
```

The highest version number is the newest backup. These are ordinary text
files, preserving comments, whitespace, line endings, and file permissions.
After a successful update, Kiroku keeps the newest **five versions for the
active monthly file**. When saving into a later month, it removes backups
for earlier months whose original files still exist as regular files. The
original monthly files are kept unchanged. Backups with missing originals
are kept for recovery, as are unrelated files in `backups/`. Cleanup also
runs on later updates, so backups left from earlier months are removed on
the next successful write. It does not back up a new file with no previous
contents, read-only commands, or operations that make no change.

The active file can still belong to a previous calendar month until you
start a new day in a later month. Commands editing that active day create
backups as usual. Every replacement of an existing monthly file creates a
backup first, regardless of the file's month or earlier backup cleanup.

The updated file and backup are flushed before the live log is replaced.
If backup creation fails, the update is aborted. Failures before replacement
leave the live file unchanged. Backups are pruned only after the updated log
and its directory have been flushed; failed saves can leave extra versions.

If replacement succeeds but the final directory flush fails, the error says
the update is **already visible**, but its persistence after a crash is
uncertain. Inspect the file or run `krk list` before retrying: repeating an
add/remove command could change the data twice. A backup cleanup failure
explicitly says the update was saved successfully and must not be repeated.
The TUI displays full errors in a popup; Enter/Esc closes it.

To recover, close the TUI and compare the monthly file with its backups in a
text editor. Copy the chosen backup's contents into the original monthly
file, keeping the original `YYYY-MM.md` filename. Kiroku never automatically
restores a backup over a live log. Backups on the same disk protect against
failed updates and unwanted changes, but are not a replacement for an
independent backup of the directory.

Optional settings live in **`~/.config/kiroku/config.toml`**, or
`$XDG_CONFIG_HOME/kiroku/config.toml` when that variable is an absolute path.
No config file is required.

```toml
data_dir = "~/Document/kiroku"
```

Change that setting to choose a persistent directory. For a single invocation:

```sh
krk --data-dir ~/other-folder list
```

Directory precedence is command-line override, saved setting, then default.
Home-relative `~/` paths are expanded; relative paths resolve against the
working directory. `krk path` prints the resolved location. Read-only commands
do not create directories or log files.

## Building and installing

Kiroku targets Linux initially. Rust 1.88 or later is needed **only to build**;
the resulting executable does not need Rust or an interpreter installed.

```sh
cargo build --release --locked
install -Dm755 target/release/krk ~/.local/bin/krk
```

For a fully static Linux x86-64 executable with no shared-library dependencies:

```sh
rustup target add x86_64-unknown-linux-musl
cargo build --release --locked --target x86_64-unknown-linux-musl
install -Dm755 target/x86_64-unknown-linux-musl/release/krk ~/.local/bin/krk
```

The repository config uses Rust's bundled `rust-lld` linker for this target.
Ensure `~/.local/bin` is in your shell's PATH. Install the binary only; the
source code, Cargo, and build toolchain are not needed to run it.

## Development checks

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test --locked
```

Tests cover rollover across weekends/months, numbered duplicate titles,
date selection and list fallback, manual comments and CRLF preservation,
malformed-file protection, stale selections, concurrent writes, configuration,
CLI workflows, search/filter references and copying results into today,
repeating items across days/months, title suggestions, and TUI
keyboard/render behavior. Save tests inject partial
writes, file/directory flush failures, replacement failures, and backup
cleanup failures, and verify the original or applied update survives as
appropriate.
