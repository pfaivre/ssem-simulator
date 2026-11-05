# AGENTS.md

Guidance for coding agents working in this repository.

## What this is

`ssem-simulator` is a command-line simulator for the SSEM ("Manchester Baby"),
the first stored-program computer. It loads a program from an assembler (`.asm`)
or binary snapshot (`.snp`) file, runs it, and prints the final machine state.
It is written in Rust (edition 2024) and uses `clap` for argument parsing and
`ratatui` for the interactive user interface.


## Build and run

```sh
cargo build
cargo run -r samples/factorct.asm
```

The CLI accepts an optional input file (`.asm` or `.snp`); without one it
starts with an empty (all-zero) store. It also takes an optional
`--max-cycles` / `-m` limit (default 100,000,000):

```sh
cargo run -r -- --max-cycles 1000 samples/nightmare.snp
```

Sample programs live in `samples/`.

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
- Don't over comment. Keep inline code comment and docstrings concise. Don't comment when the subject is obvious by reading the code.

## Suggestions for later development

- When rewinding back in the earliest state, we lose all changes we made. It would be would be more useful that the earliest state in history is the one where changes have been made.
- More themes
- more mouse interactions (changing speed, play/pause, etc.)
- first start screen
- Add CMP to data flow chart. Maybe with a label "A >= 0 ?"
- i18n, with a French translation to start with
- Fix the pop-up digit input. May work only in QWERTY and AZERTY keyboards.
