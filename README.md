# Stan Language Server

This repository is the Rust foundation for a Stan language server. The current
milestone provides a complete, versioned Stan 2.39 vocabulary and function
signature catalog plus an initial Language Server Protocol diagnostic pipeline.
The server tracks full-document updates, converts UTF-8 byte ranges to LSP UTF-16
positions, lexes source losslessly, and publishes lexical diagnostics. Parsing,
completion, semantic diagnostics, and lint rules remain later milestones.

## Workspace

- `stan-token-derive`: `#[derive(Token)]` for fixed-spelling unit enums.
- `stan-language`: language elements, lossless lexing, distributions, function metadata, and 2.39 signatures.
- `stan-language-server`: a stdio LSP server with full document synchronization and lexical diagnostics.
- `xtask`: deterministic catalog refresh and drift checking.

The workspace uses Rust edition 2024 and declares Rust 1.85 as its minimum
supported version.

## Build and test

```text
cargo build --workspace
cargo test --workspace
cargo fmt --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

Run the server over stdio with:

```text
cargo run -p stan-language-server
```

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
