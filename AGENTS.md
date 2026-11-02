# AGENTS.md

Guidance for coding agents working in this repository.

## What this is

`ssem-simulator` is a command-line simulator for the SSEM ("Manchester Baby"),
the first stored-program computer. It loads a program from an assembler (`.asm`)
or binary snapshot (`.snp`) file, runs it, and prints the final machine state.
It is written in Rust (edition 2024) and uses `clap` for argument parsing.


## Build and run

```sh
cargo build
cargo run -r samples/ssem/factorct.asm
```

The CLI accepts an optional input file (`.asm` or `.snp`); without one it
starts with an empty (all-zero) store. It also takes an optional
`--max-cycles` / `-m` limit (default 100,000,000):

```sh
cargo run -r -- --max-cycles 1000 samples/ssem/nightmare.snp
```

Pass `--save FILE` to write the final machine state to `FILE` when the run
ends, in both headless and interactive mode. In the TUI, press `S` to save
the current state at any time: the save pop-up has a single `FILE:` field,
opened on the input file's folder (canonicalized to an absolute path, shown
as the `DIR:` line) with `<name>_edited.<ext>` prefilled; Enter saves to
`<dir>/<name>`. The field accepts only alphanumerics, `-`, `_` and `.`, so
the saved file always lands in that folder.
The format follows the file extension: `.asm` writes assembly (instructions
as mnemonics, other words as `NUM`, source comments after `;`), `.snp`
writes a raw binary snapshot; CI, the accumulator and the cycle count go in
a leading comment line, and the saved file loads back as a regular input.

The interactive TUI records an execution history for time-travel (scrub the
timeline bar with the mouse, or use `←` `→` `Home` `End`), capped by
`--history-size` past states (default 100,000, 0 to disable). Headless mode
never records.

The source comments shown next to each store word (C toggles the column) are
editable: double-click the comment column of a store row to get an inline
text box on that row, prefilled with the current comment; Enter validates
(the new text becomes the word's comment, a blank text clears it) and Esc
cancels. The edit writes through `Store::set_comment` (the comments live
behind an `Arc`, swapped with `Arc::make_mut` so in-flight snapshots keep the
old text), and is included when the state is saved with `S`/`--save`.

All interface colors live in `src/tui/palette/` — a `Palette` struct plus
one file per theme (`dark.rs`, `light.rs`, `monokai.rs`, `macchiato.rs`,
`terminal.rs`; all but `terminal.rs` use exact RGB hex values, `terminal.rs`
the terminal's own default/ANSI colors). Pick a theme with `--theme
dark|light|monokai|macchiato|terminal` or cycle at runtime
with the `T` key. To add a theme: new file in `src/tui/palette/`, a `Theme`
variant, an entry in `Theme::ALL`.

The interactive view modes (theme, flow, heatmap, comments toggle, bit
representation) are
persisted in TOML to `ssem-simulator/config.toml` in the platform's config
directory — `$XDG_CONFIG_HOME` (default `$HOME/.config`) on Linux/macOS,
`%APPDATA%` (default `%USERPROFILE%\AppData\Roaming`) on Windows — written
when the TUI exits and read at startup (`src/tui/config.rs`); `--theme` overrides the saved theme
for one session without saving it. Holding a view-mode key (T/F/M/C/D) does
not repeat it: `App::mode_key_accepted` in `app.rs` ignores a press within
50 ms of the previous press of the same key, which swallows OS key-repeat on
every terminal (no kitty-protocol flags are pushed — they broke key delivery
on some terminals). The history-scrub keys (← → Home End) still repeat for
continuous time-travel.

Sample programs live in `samples/ssem/`.

## Test

```sh
cargo test
```

This runs the unit tests embedded in the `src/` modules (the same command CI
runs in `.github/workflows/rust.yml`).

## Coverage

```sh
cargo tarpaulin --skip-clean --out xml --out stdout 2>&1
```

This runs the unit tests embedded in the `src/` modules and ouputs its
coverage.

## Lint

```sh
cargo fmt --check
cargo clippy
```

## Conventions

- Keep the codebase warning-free: run `cargo clippy` before committing.
- Match the existing style (snake_case, module-per-concern under `src/ssem/`).
- Don't commit changes to `Cargo.lock` dependency version bumps unless asked.

## Suggestions for later development

- Fix the pop-up digit input. May work only in QWERTY and AZERTY keyboards.
- When rewinding back in the earliest state, we lose all changes we made. It would be would be more useful that the earliest state in history is the one where changes have been made.
- Close pop-ups by clicking outside
- More themes
- more mouse interactions (changing speed, play/pause, etc.)
- first start screen
- Add CMP to data flow chart. Maybe with a label "A >= 0 ?"
