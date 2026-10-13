# Kiroku

A fast terminal work log. The command is **`krk`**. Log short titles in
**To do**, **Doing**, and **Done**, grouped by date in editable monthly
Markdown files. No database and no timestamps on items.

## Getting started

```sh
krk todo "weekly meeting"
krk todo "monthly op meeting - september"
krk list
krk doing 1
krk list
krk done 2
krk
```

Item numbers can change after moves or removals. Default changes print the
updated active day; use the numbers from that output for the next command.
Numbers are unique across all three sections of the active day.

Run `krk --help` or `krk COMMAND --help` for command help.

Your first title addition starts today automatically if there is no active
day. Use `krk new-day` to start subsequent days and move unfinished work forward.

## Commands

| Command | Behavior |
| --- | --- |
| `krk` | Open the keyboard-driven TUI |
| `krk new-day` | Start today and move unfinished items into To do; leave an existing today unchanged |
| `krk todo "title"` | Add a todo to the active day |
| `krk list` | Show today; if empty, also show the previous recorded day |
| `krk search "text" [PERIOD]` | Search titles across all recorded days, or a selected period |
| `krk list [PERIOD] --search "text"` | List matching titles; searches all history without PERIOD |
| `krk list [PERIOD] --status STATUS` | List items with the selected status; searches all history without PERIOD |
| `krk search "text" [PERIOD] --repeat RESULT_NUMBER` | Copy one search result into active To do |
| `krk list [PERIOD] --repeat RESULT_NUMBER` | Copy one listed result into active To do; works with search/status filters |
| `krk list [PERIOD] --remove RESULT_NUMBER` | Remove one listed result from its recorded day; works with search/status filters |
| `krk search "text" [PERIOD] --remove RESULT_NUMBER` | Remove one search result from its recorded day |
| `krk doing NUMBER` | Move an active-day item into Doing |
| `krk doing "title"` | Add an item directly to Doing |
| `krk done NUMBER` | Move an active-day item into Done |
| `krk done "title"` | Add an item directly to Done |
| `krk todo NUMBER` | Move an active-day item back into To do |
| `krk todo\|doing\|done NUMBER --date YYYY-MM-DD` | Move an active-day item to that date and section |
| `krk todo\|doing\|done LETTER --from YYYY-MM-DD --date YYYY-MM-DD` | Move an item from a specific source day to that date and section |
| `krk repeat NUMBER` | Copy an active-day item into To do, keeping the original |
| `krk repeat --from YYYY-MM-DD LETTER` | Copy a historical item into the active day's To do |
| `krk move NUMBER up\|down [--steps N]` | Move an active-day item within its section (default: one place) |
| `krk edit NUMBER "title"` | Rename an active-day item |
| `krk remove NUMBER` | Remove an active-day item immediately |
| `krk path` | Print the resolved log directory |

All status commands can move an item from any section. Titles must be
nonempty, single-line text; Unicode and duplicate titles are supported.
`todo`, `doing`, and `done` treat an argument containing only ASCII digits as an item number,
even when quoted; all other text is a new title. Invalid item numbers produce
an error. Quote titles containing spaces.

### Short commands

Every command has a short alias. Aliases accept the same arguments and
options as the full command and appear in `krk --help`.

| Command | Alias |
| --- | --- |
| `list` | `l` |
| `search` | `s` |
| `todo` | `t` |
| `doing` | `do` |
| `done` | `d` |
| `new-day` | `n` |
| `repeat` | `r` |
| `edit` | `e` |
| `move` | `m` |
| `remove` | `x` |
| `path` | `p` |

For example, `krk l week`, `krk t "urgent task"`, and `krk m 3 up -n 2`.

### Short options

Every command option also has a short form. Global options work before or
after the command; other options apply to the commands that support them.

| Long option | Short option |
| --- | --- |
| `--position` | `-p` |
| `--date` | `-d` |
| `--repeat` | `-r` |
| `--remove` | `-x` |
| `--from` | `-f` |
| `--status` | `-s` |
| `--search` | `-S` |
| `--steps` | `-n` |
| `--data-dir` | `-D` |
| `--color` | `-c` |
| `--help` | `-h` |
| `--version` | `-V` |

Short options are case-sensitive. For example:

```sh
krk todo "urgent task" -p 1
krk list -S "meeting" -s done
krk repeat -f 2026-10-02 B -d 2026-10-05 -p 1
krk move 3 up -n 2
krk -D ~/other-folder -c never list
```

### Command output colors

Commands use color automatically when writing to a terminal: cyan dates and
result numbers, blue **To do**, yellow **Doing**, green **Done**, dimmed hints,
and red error prefixes. Item titles keep the terminal's default color.
Piped or redirected output stays plain, as does `krk path`.

Use the global `--color auto|always|never` option to control command output:

```sh
krk list --color always
krk --color never search "meeting"
```

In auto mode, a nonempty `NO_COLOR` environment variable or `TERM=dumb`
disables colors. Explicit `--color always` overrides these settings.
This option applies to command output; the interactive TUI keeps its own styling.

### Adding or moving to a specific date

Use `--date YYYY-MM-DD` on a command that adds a title, moves an item, or copies it:

```sh
krk todo "weekly meeting" --date 2026-10-06
krk doing "investigate issue" --date 2026-10-06
krk done "finished yesterday" --date 2026-10-05
krk doing 3 --date 2026-10-05
krk done B --from 2026-10-02 --date 2026-10-05 --position 1
krk repeat 3 --date 2026-10-05
krk repeat --from 2026-10-02 B --date 2026-10-05 --status doing
krk search "meeting" --repeat 2 --date 2026-10-05
krk list 2026-10-02 --repeat 1 --date 2026-10-05
```

The date selects the destination for this command. If it has no record yet,
Kiroku creates that day with empty sections and inserts the item, without
carrying over unfinished work or creating intervening days. Dates remain
chronological in the appropriate monthly file. You can add to past or future
dates, including when no day has been started. Moves and copies still need
a recorded source. Existing comments and line endings are preserved, with
the usual backups and storage-version checks.

Without `--date`, commands use the latest recorded day **on or before today**.
Future records are excluded from the active-day selection until their dates
arrive. Adding to an older or future date leaves the current default
destination unchanged. If there is no active day, adding a title without
`--date` creates today, including when only future records exist. Commands
that use active-day item numbers still require an active day. An explicit
`--from` can select a recorded source even when only future days exist.
`new-day` can start today even when future records already exist, and moves
items only from an earlier
eligible day. A day already filled using `--date` is left unchanged.

On `todo`, `doing`, and `done`, a numeric argument with `--date` moves that
active-day item into the chosen date's section and removes its source entry.
Use `--from YYYY-MM-DD LETTER` to select an item from a specific source day;
letters are scoped to that date, with A as the first item, B as the second,
and AA after Z. On status commands, `--from` requires `--date` and a letter.
Omit `--position` to append, or choose a one-based position in the destination
section. Moving to the same date changes the section without creating a copy;
choosing the same date and section without `--position` makes no changes.

`--date` also works with `repeat`, and with `list`/`search` when `--repeat`
is present. On moves and repeats, `--from` selects the source and `--date`
selects the destination; without `--from`, the source number refers to the
active day.
An explicit destination can precede or equal the source date. Date-targeted
commands print the updated destination with plain bullets. Use `krk list`
to get current item numbers or copy-result numbers. `edit` and `remove` continue
to act on the active day. Creating a later destination on or before today
makes it the active day under the usual selection rule. The TUI also excludes
future dates from the active day; its `m` prompt can explicitly move items
from any recorded day to a chosen date and section.

Moves within one monthly file use one atomic save. Cross-month moves prepare
both logs and their backups, then save and flush the destination before
removing the source entry. An interrupted cross-month move can leave the
item in both places. If a save reports partial completion, inspect both dates
before retrying; remove any unwanted duplicate after checking the result.

### Reusing a title

Repeating an item creates a new entry with its exact title and leaves the
original entry and status intact. To reopen an active-day Done item instead,
use `krk doing NUMBER` or `krk todo NUMBER`.

```sh
krk repeat 3
krk list 2026-10-02
krk list 2026-10-02 --repeat 2
krk repeat --from 2026-10-02 B --status doing
```

Without `--from`, use a current active-day number from an unfiltered
`krk list`. To copy from history, use `krk list YYYY-MM-DD --repeat N`
with the displayed bracketed result number. `repeat --from` also accepts
date-scoped letters: A is the first item, B the second, and labels past Z
continue as AA, AB, etc. These letters appear in the TUI's historical views.
The source may be in any section.
`--status todo|doing|done` selects the new entry's status; the
default is To do. Without `--date`, copies go to the active day,
including when it is earlier than today, and do not create a day;
run `krk new-day` first if there are no recorded days. Each repeat creates
another entry, even if the active day already contains that title.

### Days and rollover

Opening Kiroku, listing items, and crossing midnight do **not** start a day.
Without `--date`, additions and changes apply to the latest recorded day
on or before today, even after a weekend or across a month boundary. Dates use your local
timezone. When no active day exists, adding a title creates today and saves
the item in one update. This works with `todo`, `doing`, `done`, and the TUI's
`a` key. Cancelling an add prompt leaves the log unchanged.
The first day started through `new-day` starts empty.

Starting a new day moves the previous recorded day's To do and Doing items
into the new day's To do section, preserving their order and duplicate titles.
Their source entries are removed, so only Done items remain on the previously
active day. Its empty To do and Doing headings and comments stay in place.
Other earlier records remain intact; skipped days are not created. Starting
an already recorded today leaves all records unchanged. `new-day` always
starts today; date-targeted additions create missing records without rollover.

Within a month, rollover updates both days in one atomic save. Across months,
it uses the same destination-first saving as explicit item moves: both logs
and backups are prepared before replacement, and the new day is flushed
before unfinished items are removed from the source. If a cross-month save
reports partial completion, inspect both dates before retrying; interrupted
rollover may leave unfinished items in both places.

If today has not been started, `krk list` says so and shows the latest recorded
day on or before today. If today exists but is empty, it shows both today
and the most recent earlier record. Output always labels dates and the active
day. CLI listings use bracketed result numbers across the displayed days.
Updated active-day output and the TUI use bracketed item numbers; historical
TUI views use bracketed letters (`[A]`…`[Z]`, `[AA]`, etc.). Item titles align
even when labels have different lengths. Use `--repeat N` on a listing or
search to copy a historical entry, or `--remove N` to delete it.

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
sections without matches. Every CLI search or listing gives each entry a
result number in brackets: `[1]`, `[2]`, etc., counted across all displayed
days. Each entry shows just one label, for example `[2] weekly meeting`.
Titles line up across single- and multi-digit labels. Repeated titles remain
separate occurrences. Search and multi-day result numbers select entries
for `--repeat` and `--remove`; run `krk list` without filters to get active-day
item numbers for the separate editing and status commands.

To reuse a result, arrow up in your terminal to recall the same command and
append `--repeat N`, using the bracketed result number:

```sh
krk search "meeting"
krk search "meeting" --repeat 2
krk list month --search "meeting"
krk list month --search "meeting" --repeat 2
```

`--repeat` also works with an unfiltered `krk list`, or with a status filter.
It copies exactly one result into the active day's To do (or the destination
selected by `--date`), preserving the source entry and exact title, and prints
the copied source and updated destination. `--status` on a search/list
command still filters source items; the copy always starts in To do.
Invalid or missing result numbers make no
changes. The separate `krk repeat` command uses an active-day item number
or a date-scoped historical letter. These can differ from search or multi-day
result numbers.

To remove a result, recall the same command and append `--remove N` (or
`-x N`), using its bracketed result number:

```sh
krk list month --status done
krk list month --status done --remove 2
krk list --remove 3
krk search "meeting" --remove 1
```

Removal is immediate and deletes exactly that occurrence from its source
date, including historical or future dates. It prints the removed title and
date, then refreshes the same listing or search with updated result numbers.
It never starts a day. `--remove` cannot be combined with `--repeat`, `--date`,
or `--position`; the listing's period and filters select the source.
Invalid result numbers leave logs unchanged. The separate `krk remove NUMBER`
command continues to use active-day item numbers.

Results are recomputed every time the command runs. If log contents, filters,
or relative dates change between runs, the same result number may select a
different entry. Start today before your initial search if you want copies
to go into today. Without `--repeat` or `--remove`, searches and listings are
read-only and never start a day or change log files. With `--repeat`, they
create a missing day only when an explicit `--date` destination is supplied.

The separate repeat command remains available to copy a historical result
into today using its source date and letter:

```sh
krk new-day
krk search "weekly meeting" --status done
krk repeat --from 2026-10-02 B
```

Use the date and letter from your results. Start today before searching so
the displayed references reflect the active day you intend to copy into.

## Choosing an insertion position

Use `--position N` when adding a title, moving an item into a section, or
copying an item. Positions start at **1 within the destination section**,
independently of the item numbers shown by `krk list`. Existing items at
that position and later shift down. Omitting the option appends as before.

```bash
krk todo "urgent task" --position 1
krk doing 3 --position 2
krk repeat 4 --status done --position 1
krk todo "backfill" --date 2026-10-05 --position 2
krk search "meeting" --repeat 1 --position 1
krk list yesterday --repeat 1 --position 1
```

For a section containing three items, valid insertion positions are 1–4;
4 appends. An empty section accepts only 1. Out-of-range positions fail
without saving changes. When moving an item within its current section,
the position specifies its final place after removing its old entry.
`--position` also works with `--date`; on `list` and `search` it requires
`--repeat`.

In the terminal add prompt, use Ctrl-←/→ to choose an insertion position
in To do. The prompt displays the chosen position and defaults to appending.
↑/↓ and Tab still select previous titles.

## Keyboard navigation

| Key | Action |
| --- | --- |
| ↑/↓ or `j`/`k` | Select an item |
| `K`/`J` (Shift-k/j) | Move the selected item up/down within its section |
| ←/→ or `h`/`l` | Browse earlier/later recorded days |
| Home/End or `g`/`G` | Select the first/last item |
| `a` | Add a todo |
| `c` | Copy the selected item from any day into the active day's To do |
| `m` | Move the selected item to a chosen date and section |
| `/` | Search titles across all recorded days |
| `s`, `d`, `t` | Move to Doing, Done, or To do |
| `e` | Edit the selected title |
| `x` | Remove the selected item, with a `y`/`n` confirmation |
| `n` | Start today explicitly |
| `r` | Reload manually edited files |
| `?` | Show help |
| `q` or Ctrl-C | Quit |

Reordering works in To do, Doing, and Done on the active day. It saves the
order in the Markdown file and keeps the moved item selected. At the first
or last item in a section, moving farther does nothing; it never changes
an item’s status. Item numbers update to reflect the new order.
Use `krk move 3 up --steps 2` to move item 3 past two items in one save.
`--steps` must be a positive integer; moves larger than the available space
stop at the first or last item in that section.

Copying with `c` preserves the source, jumps to the active day, and selects
the new entry. Press `s` to start working on it immediately.

Press `m` to move the selected item, including from a historical or future
day. The prompt starts with today's date and the item's current section.
Type the destination as `YYYY-MM-DD` (Ctrl-U clears the date), and use
Tab/Shift-Tab to choose To do, Doing, or Done. Enter moves the item and selects
it on the destination day; Esc cancels. A missing destination starts empty,
without carryover. The source entry is removed. The move appends to the
chosen section and rejects a source selection changed by an external edit.

Press `/`, enter search text, and press Enter to list matches across all
days, newest day first. An empty query lists all items. Results show the
source date, original number/letter, status, and title. Navigate with
Up/Down or `j`/`k`; Home/End or `g`/`G` selects the first/last match.
Press `f` to cycle the status filter through all, Done, To do, and Doing.
Press `/` again to refine the query while retaining the status filter.
Enter opens the selected item's source day, `c` copies it into active To do,
`m` opens the move prompt, and Esc leaves the results. To copy into today, press `n` first; this starts
today while retaining the selected search result. `r` refreshes results
after external edits. Opening an active-day result also lets you use the
usual editing and status shortcuts; use `m` for explicit moves from other days.

In a title prompt, type or paste a title, use Backspace to delete a character
or Ctrl-U to clear, Enter to save, and Esc to cancel. The add prompt shows up
to five matching previous titles, most recently used first. Each exact title
appears once, with its most recent completion date (or last use if never
completed). Matching ignores case and searches anywhere in the title. Use
Up/Down or Tab/Shift-Tab to select a suggestion, then Enter to reuse its exact
text. With no suggestion selected, Enter saves the text you typed. Typing or
deleting clears the suggestion selection. Renaming, removing, reordering,
and the `s`/`d`/`t` status shortcuts apply only to the active day. Use `m` to
explicitly move other days' items, or edit their Markdown files directly. It redraws
on input and resize rather than running an idle animation loop.

## Files and configuration

Default logs live in **`~/Documents/kiroku/`**.
Each month has a file such as `2026-10.md`:

```markdown
<!-- kiroku-format: 1 -->

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

### Storage versions and compatibility

The monthly Markdown format is **version 1**, independent of the app release
shown by `krk --version`. New files start with `<!-- kiroku-format: 1 -->`.
The header applies to the entire file and appears only once, before the first
date heading. Leading blank lines and ordinary comments are allowed. Comments
beginning with `kiroku-format` after the opening `<!--` are reserved for this
single-line header; the version must be a positive decimal integer.

Existing files without a header are validated as format 1. Reading them does
not change them. Their next actual update adds the header while preserving
untouched content and the LF/CRLF newline convention; the usual backup keeps
the exact previous bytes. No-op commands do not add a header or create a
backup. Historical files remain unmodified unless they are updated.

Kiroku rejects unsupported versions and malformed, duplicate, or misplaced
headers whenever it reads a file. A newer version reports the file and line
and asks you to upgrade Kiroku before reading or changing it. Do not remove
or lower a version header to bypass this check. Builds predating version
checks ignore HTML comments and cannot provide this protection.

Format 1 needs no data conversion. Future format changes will require
documented compatibility and migration steps, with validation, repeatable
execution, recovery after interruption, and migration snapshots retained
separately from the rotating `backups/` directory. No migration command or
automatic conversion is implemented yet. App releases that retain format 1
do not need to migrate logs. Configuration has its own TOML structure and is
not versioned by the monthly-file header.

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
updated monthly file**. When a default active-day write or `new-day` saves
into a later month, it removes backups for earlier months whose original
files still exist as regular files. The
original monthly files are retained; rollover removes unfinished source
entries after saving them in the new month. Backups with missing originals
are kept for recovery, as are unrelated files in `backups/`. Cleanup also
runs on later updates, so backups left from earlier months are removed on
the next successful active-day write. Explicit `--date` updates retain other
months' backups, so planning a future month does not discard active-month
backups. It does not back up a new file with no previous contents, read-only
commands, or operations that make no change.

The active file can still belong to a previous calendar month while the
latest record on or before today remains there. Commands editing that active day create
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
data_dir = "~/Documents/kiroku"
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

From a cloned checkout, install with one command:

```sh
./install.sh
```

This builds the release executable and installs `krk` into `~/.local/bin`,
replacing an existing installation there. Run it again after pulling updates.
The script uses the local Rust toolchain when available, otherwise Docker,
and finally Podman if Docker is not installed.
The script reports if that directory needs to be added to your PATH. It does
not need sudo or change your logs or configuration.

To explicitly select Docker or Podman, use:

```sh
./install.sh --docker
# Or, with Podman:
./install.sh --podman
```

Container builds require a Linux x86-64 or ARM64 host. Docker needs a running
daemon accessible to your user; Podman supports its usual rootless setup.
The script pulls the official `docker.io/library/rust:1.88-alpine` image, builds a
static Linux executable inside a temporary container, and installs it into
the same `~/.local/bin` directory. The container engine and Rust are not needed to run the
installed tool. The temporary container and build files are removed; the
downloaded image stays available for later installs. Build inputs are copied
into the container, so the checkout is not modified by the container.

Use `./install.sh --help` for the available options.

To uninstall:

```sh
./install.sh --uninstall
```

This removes only `~/.local/bin/krk`, preserving logs and configuration.
It requires no Rust toolchain or container engine and is safe to repeat if
the executable is already absent.

Alternatively, build and install manually:

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

Tests cover moving unfinished items during rollover across weekends/months,
leaving only Done items on the source day, numbered duplicate titles,
date selection and list fallback, manual comments and CRLF preservation,
legacy and versioned logs, unsupported-format protection, malformed-file
protection, stale selections, concurrent writes, configuration,
date-targeted additions, moves, and copies with chronological insertion,
CLI workflows, search/filter references and copying results into today,
numbered search/list results with `--repeat` and `--remove`, repeating items across
days/months, title suggestions, and TUI
keyboard/render behavior. Save tests inject partial
writes, file/directory flush failures, replacement failures, and backup
cleanup failures, and verify the original or applied update survives as
appropriate. Cross-month move tests cover failures while preparing either
log, partial completion, and concurrent moves without lost items.
