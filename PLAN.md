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
- Default data directory: `~/Document/kiroku/` (singular `Document`).
  Users can choose a different directory for their log files.
- Store optional configuration in `~/.config/kiroku/config.toml`, or
  `$XDG_CONFIG_HOME/kiroku/config.toml` when `XDG_CONFIG_HOME` is set.
  Without a configuration file, use the defaults.
- A `data_dir` setting selects a persistent custom data directory, supporting
  a home-relative value such as `data_dir = "~/Document/kiroku"`.
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
- Until a new day is manually started, additions and status changes apply
  to the last started day, including after midnight or a gap of several days.
  This is the initial behavior and can be revisited after use.
- The first day starts empty.
- Starting a subsequent day copies the previous recorded day's To do and
  Doing items into the new day's To do section.
- The new day's Doing and Done sections start empty.
- Move items from To do to Doing, then from Doing to Done.
- Done items are not carried forward.
- Preserve previous days' records when copying unfinished items forward.
- CLI commands select items by a number displayed in `krk list`, allowing
  duplicate titles to be selected separately (for example, `krk doing 3`).
- Initial commands: `krk new-day`, `krk add "title"`, `krk list`,
  `krk doing NUMBER`, `krk done NUMBER`, `krk todo NUMBER`,
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

## Proposed interface and storage

These choices are proposals to settle before implementation.

- `krk new-day` starts today's record.
- Provide `krk path` to print the resolved data directory. This command
  remains a proposal.
- Generate item numbers from the active day's list rather than storing IDs
  alongside titles. Use unique numbers across all three sections of that day.
  Numbers may change after edits or moves; show the updated list after changes.
- Use numbers for the active (last manually started) day's items and letters
  for historical items, labeling the active day explicitly. Before starting
  today, the previous recorded day remains active and retains its numbers.
  After starting an empty today, the preceding day is historical and receives
  letters. Historical labels are for browsing; mutation commands accept only
  active-day numbers. This numbering scheme is a proposal.
- Proposed selectors: `krk list today`, `krk list yesterday`,
  `krk list week`, `krk list month`, `krk list YYYY-MM-DD`, and
  `krk list YYYY-MM-DD..YYYY-MM-DD` (inclusive range).
- Interpret today/yesterday as calendar dates in the user's local timezone,
  week as the current calendar week (Monday through Sunday), and month as the
  current calendar month. These period definitions are proposals.
- Explicit date selectors show only the requested dates, with no fallback
  to a different period. Multi-day views group items by date in chronological
  order and show recorded days, including empty records. Report when the
  selected period has no records. These display details are proposals.
- The TUI supports browsing days, navigating items, adding titles, and
  changing status with keyboard shortcuts.
- If days are skipped, carry unfinished items from the most recent recorded
  day, without creating intervening files.
- If today's record already exists, the new-day command leaves it intact
  and reports that the day has already been started.

## Recommended technology

Rust, compiled in release mode to a native executable named `krk`.
Rust's toolchain is needed to build the app, but is not needed to run the
distributed executable. Produce binaries for each supported operating system
and architecture; for Linux, target musl for a self-contained static build.

Use Ratatui with Crossterm for the TUI. Both are compiled into the executable.
Keep the CLI direct and lightweight, and redraw the TUI on input or resize
rather than running a continuous animation loop. Read only the log files
needed for the current operation.

This technology choice is a recommendation pending agreement, not an
implementation already started.

## Decisions still to make

- Whether starting a day for a supplied date is supported.
- Confirm active-day numbers versus historical letters, including labeling
  large multi-day listings and whether app-based historical editing is needed.
- Confirm date selector syntax and calendar week/month definitions.
- Exact text format, including preserving manual edits.
- Reordering items and the details of editing/removal.
- Confirm the recommended technology and supported operating systems.
- Additional list options and TUI key bindings.

## Status

Planning only. No implementation has been started.
