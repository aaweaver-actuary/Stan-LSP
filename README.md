# Stan Language Server

This repository is a beta-focused Stan development suite built around a complete,
versioned Stan 2.39 vocabulary and function signature catalog. Local analysis is
responsive and conservative; stanc3 remains the semantic authority.
The server tracks full-document updates, converts UTF-8 byte ranges to LSP UTF-16
positions, builds a lossless recovery syntax model, and publishes lexical and
structural diagnostics. It also provides scoped symbols and references, folding,
semantic tokens, hover/completion/signature help, native formatting, configurable
lint infrastructure, user-defined probability functions, and optional stanc3
validation on save. Partial semantic features and native formatting remain
experimental until they pass the documented corpus gate.

## Workspace

- `stan-token-derive`: `#[derive(Token)]` for fixed-spelling unit enums.
- `stan-language`: language elements, lossless lexing, distributions, function metadata, and 2.39 signatures.
- `stan-language-server`: a stdio LSP server with full document synchronization and lexical diagnostics.
- `stan-tools`: `stanfmt` and `stanlint` command-line tools.
- `xtask`: deterministic catalog refresh and drift checking.

The workspace uses Rust edition 2024 and declares Rust 1.85 as its minimum
supported version.

## Build and test

```text
cargo build --workspace
cargo test --workspace
cargo test --doc --workspace
cargo fmt --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
cargo run -p xtask -- corpus check
```

Run the server over stdio with:

```text
cargo run -p stan-language-server
```

Format or check models with:

```text
cargo run -p stan-tools --bin stanfmt -- model.stan
cargo run -p stan-tools --bin stanfmt -- --check model.stan
cargo run -p stan-tools --bin stanlint -- model.stan
```

The VS Code client is under `editors/vscode`. Start with the
[repository map](docs/repository-map.md), [analysis pipeline](docs/analysis-pipeline.md),
[semantic model](docs/semantic-model.md), [configuration](docs/configuration.md),
and [diagnostic codes](docs/diagnostics.md).

## Corpus validation

Ordinary builds use a project-owned offline smoke corpus. Maintainers can fetch
the checksum-pinned external Stan example corpus and compare formatting and local
diagnostics with stanc3:

```text
cargo run -p xtask -- corpus fetch
cargo run -p xtask -- corpus check --external --stanc /path/to/stanc --json corpus-report.json
```

See the [testing guide](docs/testing.md) and
[release checklist](docs/release-checklist.md) for the beta-quality gates.

## Refresh the Stan catalog

Normal builds are offline with respect to Stan and do not require the manuals or
`stanc`. Maintainers can regenerate the checked-in signature snapshot using an
official stanc3 2.39.0 binary:

```text
cargo run -p xtask -- refresh-stan --stanc /path/to/stanc
cargo run -p xtask -- refresh-stan --stanc /path/to/stanc --check
```

The tool verifies the compiler version, normalizes and deduplicates the compiler
signature dump, synthesizes unnormalized probability overloads, preserves explicit
higher-order templates, and regenerates the auditable token enums.

See [docs/sources.md](docs/sources.md) for provenance and catalog policy.

## Licensing

The project is available under the MIT or Apache-2.0 license, at your option.
The Stan documentation and compiler sources retain their respective upstream
licenses; see the provenance document.
