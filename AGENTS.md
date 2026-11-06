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
- Don't over comment. Keep inline code comment and docstrings short and concise. Don't comment when the subject is obvious by reading the code.
- Do not let changes push a file from under 1k lines to over 1k lines without a very strong reason. Prefer extracting helpers, subcomponents, modules, or local abstractions. If the diff crosses that threshold, explicitly ask whether the code should be decomposed first. Only waive this with a compelling structural reason where the file is still clearly organized.
- Bias toward cleaning the design, not just accepting working code. Strongly prefer simplifications that remove moving pieces altogether over refactors that merely spread the same complexity around. Do not rubber-stamp "it works" implementations that leave the codebase messier.
- Prefer direct, boring, maintainable code over hacky or magical code. Flag brittle ad-hoc behavior, generic mechanisms hiding simple data-shape assumptions, thin abstractions, identity wrappers, and pass-through helpers that add indirection without buying clarity.

## Localisation

- Interface strings go through `rust_i18n`: `t!("key")` (returns `Cow<str>`, for
  inline rendering) or `ts!("key")` (owned `String`, for fields/return types),
  with `%{name}` placeholders filled by `name = value` args (a format spec can
  follow: `n = 5 : {:02}`).
- Catalogs live in `locales/<code>.toml` (`en` is the source of truth); add one
  file per language and a `Language` variant. `src/i18n.rs` resolves the active
  language once in `main()` (`--lang` > `language` config key > `LC_ALL` /
  `LC_MESSAGES` / `LANG` > `en`) — never lazily inside `t!`, so tests always run
  in English. Tests enforce catalog parity and that every key used in code
  exists.
- Not translated: opcode mnemonics, the `NUM` file keyword, register names
  `CI`/`A`, file extensions, config keys, `.asm` comments.

## Suggestions for later development

- More themes
- more mouse interactions (changing speed, play/pause, etc.)
- first start screen
- Add CMP to data flow chart. Maybe with a label "A >= 0 ?"
- Fix the pop-up digit input. May work only in QWERTY and AZERTY keyboards.
