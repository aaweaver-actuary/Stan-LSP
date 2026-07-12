# Beta release checklist

- Rust 1.85 and stable build, test, doc-test, format, Clippy, and Rustdoc jobs pass.
- Generated catalogs validate without drift.
- Exact protocol tests pass on Linux, macOS, and Windows.
- Parser, analysis, formatter, and range panic counts are zero.
- Semantic invariant failures are zero.
- Formatting is idempotent and preserves stanc3 validity across the pinned corpus.
- Default local error diagnostics on stanc-valid models are zero.
- Remaining hint false positives are recorded in the corpus report.
- Diagnostic IDs, configuration, repository map, and supported versions are current.
- Release artifacts contain reproducible version metadata and checksums.
- The corpus report is attached to the release notes.
