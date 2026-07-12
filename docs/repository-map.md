# Repository map

| Concern | Primary location | Important tests | Notes |
| --- | --- | --- | --- |
| Fixed Stan vocabulary and catalogs | `crates/stan-language/src/catalog.rs`, generated files | `function_catalog.rs`, `syntax_catalog.rs` | Generated files must be changed through `xtask`. |
| Lossless lexing | `crates/stan-language/src/lexer.rs` | lexer unit tests and corpus checks | Tokens use UTF-8 byte ranges and reconstruct the source. |
| Recovery parsing and typed syntax | `crates/stan-language/src/parser.rs` | parser unit tests and syntax fixtures | Always returns a tree; semantic data is not stored on nodes. |
| Symbols, scopes, and resolution | `crates/stan-language/src/semantics.rs` | semantic unit/fixture tests | Conservative and declaration ordered. |
| Diagnostics and lint policy | `diagnostic.rs`, `lint.rs` | lint tests and diagnostic fixtures | stanc3 remains authoritative. |
| Formatting | `format.rs` | formatter fixtures and corpus checks | Operates on lossless analysis and must be idempotent. |
| LSP state and mapping | `document.rs`, `mapper.rs` | document, Unicode, and protocol tests | `LspMapper` is the only LSP position boundary. |
| Editor features | `stan-language-server/src/features/` | focused feature and protocol fixtures | Handlers translate protocol data; language rules stay outside the backend. |
| Compiler execution | `stan-language-server/src/stanc.rs` | stanc parser and scheduled differential tests | External semantic authority. |
| Catalog and corpus maintenance | `xtask/src` | `xtask validate`, corpus smoke tests | Normal builds remain offline. |
| VS Code client | `editors/vscode` | Node manifest tests | Thin client for the editor-neutral server. |

Catalog assets under `catalog/stan-2.39` and generated Rust catalogs must not be
edited manually. Corpus downloads are cached outside tracked source and are
identified by the provenance manifest.
