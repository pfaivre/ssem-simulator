# AGENTS.md

Guidance for coding agents working in this repository.

## What this is

`ssem-simulator` is a command-line simulator for the SSEM ("Manchester Baby"),
the first stored-program computer. It loads a program from an assembler (`.asm`)
or binary snapshot (`.snp`) file, runs it, and prints the final machine state.
It is written in Rust (edition 2024) and uses `clap` for argument parsing.

## Setup

Any command should be run inside the devenv like so:

```sh
nix shell nixpkgs#devenv -c devenv shell <command to run>
```

## Build and run

```sh
cargo build
cargo run -r samples/ssem/factorct.asm
```

The CLI accepts one input file (`.asm` or `.snp`) and an optional `--max-cycles`
/ `-m` limit (default 100,000,000):

```sh
cargo run -r -- --max-cycles 1000 samples/ssem/nightmare.snp
```

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
