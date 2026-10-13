# Kiroku plan

## Purpose

A terminal app for logging what needs to be done, what is being worked on,
and what has been finished. Each item is a short title, such as "weekly
meeting" or "monthly op meeting - september".

## Agreed requirements

- Provide both a CLI and a TUI with keyboard navigation.
- The app is called Kiroku; its executable and command are named `krk`.
- Distribute an independent native executable, without requiring a separately
  installed interpreter or language runtime.
- Prioritize fast startup, low memory use, and responsive navigation.
- Store data in plain text files, with no database.
- Default data directory: `~/Documents/kiroku/`.
  Users can choose a different directory for their log files.
- Store optional configuration in `~/.config/kiroku/config.toml`, or
  `$XDG_CONFIG_HOME/kiroku/config.toml` when `XDG_CONFIG_HOME` is set.
  Without a configuration file, use the defaults.
- A `data_dir` setting selects a persistent custom data directory, supporting
  a home-relative value such as `data_dir = "~/Documents/kiroku"`.
- `--data-dir PATH` overrides the configured directory for that invocation.
  Directory precedence: command-line override, saved setting, default.
- Keep all three states together in a human-readable, manually editable
  chronological log. Do not use separate files for To do, Doing, and Done.
- Keep multiple days in each file, rather than creating a file for every day.
- Use one Markdown log per calendar month, named `YYYY-MM.md`, from the
  beginning. Each recorded day has a date heading, three status subheadings,
  and one title per list item. No size threshold is needed initially.
- Starting the first recorded day of a new month creates its file; editing
  the last started day continues to update its existing file.
- Carry unfinished items across month/file boundaries using the same rules
  as within a file, preserving the source day's record.
- Group items by day/date. Individual items have no timestamps.
- Each day has three sections: To do, Doing, and Done.
- Start a new day only through an explicit command. Opening the app or
  crossing midnight does not start a new day automatically.
- Exception: when no active day exists, adding a title creates today and
  saves the item in one update, including when only future records exist.
  This applies to CLI title additions and the TUI add prompt; cancelling
  the prompt or submitting an invalid title does not create a day.
- Until a new day is manually started, additions and status changes apply
  to the last started day, including after midnight or a gap of several days.
  This is the initial behavior and can be revisited after use.
- CLI additions, moves, and copies can override the destination with `--date
  YYYY-MM-DD`. Create a missing destination with empty sections, without
  carryover, and insert dates chronologically. Older destinations leave the
  default active day unchanged; the active day is the latest recorded date
  on or before today. Future records are excluded until their dates arrive.
  Without an explicit destination, numbered moves and other edits target
  the active day. Status commands can move a source date's letter using
  `--from` together with `--date`. The TUI `m` prompt moves a selected item
  from any day to a chosen date and section. `new-day` always starts today.
- The first day starts empty.
- Starting a subsequent day moves the previous recorded day's To do and
  Doing items into the new day's To do section, preserving order and duplicates.
  Remove their source entries so only Done items remain on the previously
  active day, retaining its headings and comments.
- The new day's Doing and Done sections start empty.
- Move items from To do to Doing, then from Doing to Done.
- Done items are not carried forward.
- Preserve Done entries and all other earlier days during rollover.
- CLI commands select items by a number displayed in `krk list`, allowing
  duplicate titles to be selected separately (for example, `krk doing 3`).
- Initial commands: `krk new-day`, `krk todo "title"`, `krk list`,
  `krk doing TITLE_OR_NUMBER`, `krk done TITLE_OR_NUMBER`, `krk todo NUMBER`,
  `krk edit NUMBER "title"`, and `krk remove NUMBER`.
- Running `krk` without a subcommand opens the TUI.
- Default `krk list` shows today's items. If today's recorded day is empty,
  show that empty day and also the most recent earlier recorded day. If today
  has not been started, state that clearly and show the last recorded day.
  Always display dates and clearly distinguish earlier days from today.
  If no earlier record exists, report that without creating a day.
  Listing does not start a day or change the active day for edits.
- Allow selecting which day or days to list, including today, yesterday,
  week, and month.

## First-version interface and storage

These defaults were adopted for the first implementation.

- `krk new-day` starts today's record.
- Provide `krk path` to print the resolved data directory.
- Generate item numbers from the active day's list rather than storing IDs
  alongside titles. Use unique numbers across all three sections of that day.
  Numbers may change after edits or moves; show the updated list after changes.
- Use numbers for the active day's items and letters for other dates,
  labeling the active day explicitly. Active means the latest recorded date
  on or before today. Before starting
  today, the previous recorded day remains active and retains its numbers.
  After starting an empty today, the preceding day is historical and receives
  letters. Ordinary editing commands accept only active-day numbers.
  Explicit dated moves and repeats may select a historical entry using its
  date and letter from a single-date
  listing. Letters continue past Z as AA, AB, etc., and are unique across
  historical items in each CLI listing.
- Selectors: `krk list today`, `krk list yesterday`,
  `krk list week`, `krk list month`, `krk list YYYY-MM-DD`, and
  `krk list YYYY-MM-DD..YYYY-MM-DD` (inclusive range).
- Interpret today/yesterday as calendar dates in the user's local timezone,
  week as the current calendar week (Monday through Sunday), and month as the
  current calendar month.
- Explicit date selectors show only the requested dates, with no fallback
  to a different period. Multi-day views group items by date in chronological
  order and show recorded days, including empty records. Report when the
  selected period has no records.
- The TUI supports browsing days, navigating items, adding titles, and
  changing status with keyboard shortcuts.
- If days are skipped, move unfinished items from the most recent recorded
  day, without creating intervening files.
- If today's record already exists, the new-day command leaves it intact
  and reports that the day has already been started.
- `new-day` starts today only. It can insert today before existing future
  records and moves unfinished work only from an earlier eligible day.
  If today's record already exists, leave it unchanged.
- Rollover within a month updates both days in one save. Across months,
  prepare both files and backups, save and flush the destination first,
  then remove source entries. Preserve recovery backups until both files
  are durable, then apply the usual closed-month cleanup.
- `--date YYYY-MM-DD` targets title additions and item moves through `todo`,
  `doing`, and `done`, and copies through `repeat` or `list`/`search --repeat`.
  Numeric status moves select the active day's source item; `--from` selects
  a date-scoped letter and requires `--date`. Require `--repeat` for `--date`
  on search/list. Copy source selection remains independent of destination.
  Validate the source before creating a destination and hold the directory
  lock through the entire update. Print targeted results without implying
  that historical items have editable active-day numbers.
- Moving removes only the selected source entry and inserts its exact title
  into the destination section, with optional `--position`. Create missing
  destinations without carryover. Same-date moves can change section or
  position; the same section without a position is a no-op. Within one month,
  save once. Across months, prepare both files and backups before replacement,
  then flush the destination before removing the source. Report partial
  completion with instructions to inspect both files before retrying.
- All status commands may move an active-day item from any status. Editing
  replaces its title; removal deletes the item. CLI removal is immediate;
  TUI removal has a y/n confirmation.
- `krk repeat NUMBER` copies an active-day item into To do without changing
  its original entry. `krk repeat --from YYYY-MM-DD LETTER` copies from a
  different recorded date, with letters scoped to a single-date listing.
  `--status todo|doing|done` chooses the new status, defaulting to To do.
  Copying keeps the exact title and permits duplicates. Without `--date`,
  copies target the active day and never create a day. An explicit `--date`
  can create a destination without carryover.
- `krk search TEXT [PERIOD]` and `krk list [PERIOD] --search TEXT` search
  title substrings ignoring case. Both support `--status todo|doing|done`;
  `list --status` also works without text. With no period, filters search
  all recorded days. Explicit periods restrict dates without fallback.
  Results are read-only and preserve original active-day numbers and
  date-scoped historical letters so `repeat` can copy matches directly.
- CLI search and list rows also have bracketed result numbers across all
  displayed days. Appending `--repeat RESULT_NUMBER` to the same command
  copies that result into active To do and prints the source and updated
  day. Filters still select source items. Unfiltered lists support this
  option without changing their default date selection. Invalid numbers
  make no changes; copying never starts a day. Results are recomputed on
  each run, so numbers can refer to different entries after log or date
  changes. Original source labels remain available for separate commands.
- TUI `/` prompts for a search across all recorded days. Results list the
  date, original label, status, and title, newest day first. Navigation
  selects matches; `f` cycles all/Done/To do/Doing, `/` refines the query,
  Enter opens the source day, `c` copies into active To do, and Esc returns
  to browsing. `n` starts today without losing the selected result, following
  an unfinished item to the new day or retaining a Done item on its source day;
  `r` reloads results. Search copies retain stale-view protection.
- TUI `c` copies the selected item from any day into active To do, jumps to
  the active day, and selects the copy. Reject stale source and destination
  snapshots before saving. The add prompt suggests unique previous titles,
  most recently used first, with their latest completion date (or last use).
  Case-insensitive substring matching shows up to five titles; Up/Down or
  Tab/Shift-Tab selects one for Enter to reuse. Enter without a selection
  saves the typed text. Typing or deleting resets the suggestion selection.
- TUI `m` opens a move prompt in browsing or search. Prefill today and the
  source item's section; type YYYY-MM-DD, Tab/Shift-Tab cycles the section,
  Enter moves and selects the item at its destination, and Esc cancels.
  Source snapshots reject stale selections. Ordinary historical edits remain
  restricted; explicit moves may select past or future sources and destinations.
- Preserve untouched lines, previous day records, blank lines, and standalone
  HTML comments, including multiline comments. Retain the file's LF/CRLF
  newline convention. Reject malformed or unsupported content instead of
  overwriting it.
- Dates must be unique and chronological within the correct monthly file.
  Each day must have the three status headings in To do, Doing, Done order.
  Titles are nonempty single-line text; Unicode and duplicate titles work.
- Version the monthly Markdown format independently of app releases. Format 1
  uses one `<!-- kiroku-format: 1 -->` header before the first date heading.
  Validate unmarked legacy files as format 1 and annotate only on an actual
  update, preserving original bytes in the usual backup. Reads and no-op
  commands leave files unchanged. Reject unsupported formats and malformed,
  duplicate, or misplaced version headers before using the file.
- Use atomic file replacement and serialize app writes with a directory lock.
  Reject stale TUI item selections after external edits and provide reload.
- Before replacing an existing monthly file, save and flush its exact previous
  contents in `backups/YYYY-MM.md.NUMBER.bak`. Retain five versions per month,
  pruning only after the new log and its directory are successfully flushed.
  Abort the update if backup creation fails. Failed saves may retain extras.
- Explicit date-targeted updates retain other months' backups. Planning a
  future month must not prune backups of the current active month.
- Distinguish failures before replacement from failures after the update is
  visible. After a failed final directory flush, explain the uncertain
  durability and instruct the user to inspect before retrying. A cleanup
  failure reports that the update succeeded. Show full TUI errors in a popup.
- Test partial writes, flush failures, replacement failures, backup retention,
  and cleanup failures through injectable IO operations.
- TUI: arrows/jk select items, arrows/hl browse days, a adds, c copies, m moves,
  / searches, s/d/t change status, e edits, x removes, n starts today,
  r reloads, ? shows help, q quits.
  Enter saves a title prompt; Esc cancels; Backspace edits; Ctrl-U clears.

Example monthly file:

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

## Technology

Rust, compiled in release mode to a native executable named `krk`.
Rust's toolchain is needed to build the app, but is not needed to run the
distributed executable. Target Linux initially, with an x86-64 musl release
build for a self-contained static executable.

Use Ratatui with Crossterm for the TUI. Both are compiled into the executable.
Keep the CLI direct and lightweight, and redraw the TUI on input or resize
rather than running a continuous animation loop. Read only the log files
needed for the current operation.

## Deferred features

- Data conversions when a future storage format requires them. Migrations
  must validate data, be repeatable and recoverable after interruption, and
  retain separate snapshots outside ordinary rotating backup cleanup.
- Reordering items through the app (manual file editing is available).
- Historical editing through the app (manual file editing is available).
- Starting a day for a supplied date.
- Releases for other operating systems and architectures.

## Status

First implementation complete: CLI, TUI, monthly Markdown storage, and
configuration. README.md documents commands, file format, installation,
keyboard shortcuts, and verification. The application is not installed into
the user's PATH automatically.
