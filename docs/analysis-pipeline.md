# Analysis pipeline

```text
source text
  -> lossless lexer
  -> recovery-oriented syntax tree and typed views
  -> lexical scopes, declarations, references, and conservative types
  -> structural diagnostics and configurable lints
  -> immutable AnalysisSnapshot
  -> protocol-independent features
  -> UTF-16 LSP conversion
```

Every accepted document revision is fully recomputed and stored as one immutable
snapshot. Features do not lex or parse independently. UTF-8 byte ranges are used
inside the language crate; conversion to UTF-16 happens only through `LspMapper`.

On save, or after an enabled debounce, stanc3 runs asynchronously against the
matching document revision. Obsolete runs are cancelled and stale results are not
published. Local and compiler diagnostics are combined conservatively: exact
range-and-message duplicates are removed, while uncertain overlaps remain visible.
