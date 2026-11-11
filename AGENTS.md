# AGENTS.md

Guidance for coding agents working in this repository.

## What this is

`ssem-simulator` is a command-line simulator for the SSEM ("Manchester Baby"),
the first stored-program computer. It loads a program from an assembler (`.asm`)
or binary snapshot (`.snp`) file, runs it, and prints the final machine state.
It is written in Rust (edition 2024) and uses `clap` for argument parsing,
`ratatui` for the interactive user interface and `rust-i18n` for localization.


## Build and run

```sh
cargo build
cargo run -r samples/factorct.asm --headless
```

The CLI accepts an optional input file (`.asm` or `.snp`); without one it
starts with an empty (all-zero) store. It also takes an optional
`--max-cycles` / `-m` limit (default 100,000,000):

```sh
cargo run -r -- --max-cycles 1000 samples/nightmare.snp --headless
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

## Checks

```sh
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
```

## Conventions

- Keep the codebase warning-free: run checks before committing.
- Match the existing style (snake_case, module-per-concern under `src/ssem/`).
- Don't over comment. Keep inline code comment and docstrings short and concise. Don't comment when the subject is obvious by reading the code.

## Review principles (thermos)

Branch audits run a bug/security pass and a code-quality pass over the diff.
Write new code as if it will be audited against these rubrics:

- Be ambitious about simplification ("code judo"): prefer the restructure
  that deletes whole branches, helpers, modes, or layers over one that merely
  moves the same complexity around. If a path exists to remove complexity
  rather than rearrange it, take it.
- No spaghetti growth: ad-hoc conditionals or one-off special cases bolted
  onto unrelated flows are a design problem, not a stylistic nit — push the
  logic into a dedicated helper, state machine, policy object, or module.
- Type and boundary cleanliness: no unnecessary `Option`, `any`, or casts
  where a clearer type boundary exists; explicit typed models over
  loosely-shaped ad-hoc objects; no silent fallbacks papering over unclear
  invariants.
- Keep logic in its canonical layer: no feature-specific code leaking into
  shared paths, no bespoke one-offs where a canonical helper already exists,
  and related state updates atomic rather than spread across a flow.
- Correctness first: no panics on user-controlled input; error paths return
  `Result` and are handled at the boundary; every behavior change is
  deliberate and covered by a test.
- "It works" is not approval. Presumptive blockers unless justified: a file
  pushed from under to over 1k lines, scattered feature checks in shared
  code, an added indirection layer that clarifies nothing, duplicated logic
  instead of an extracted helper.
- Do not let changes push a file from under 1k lines to over 1k lines without
  a very strong reason. Prefer extracting helpers, subcomponents, modules, or
  local abstractions. If the diff crosses that threshold, explicitly ask whether
  the code should be decomposed first. Only waive this with a compelling
  structural reason where the file is still clearly organized.

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

- Click on statusbar
- first start screen
- Add CMP to data flow chart. Maybe with a label "A >= 0 ?"
- Fix the pop-up digit input. May work only in QWERTY and AZERTY keyboards.
