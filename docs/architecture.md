# Architecture

Source text is converted into a revisioned immutable `AnalysisSnapshot` containing
lossless tokens, a recovery-oriented concrete syntax tree, and transport-neutral
diagnostics. Editor features, `stanfmt`, and `stanlint` consume the same snapshot.

The language-server crate owns URI/document state, UTF-16 conversion, protocol
adaptation, and external stanc3 execution. It does not own Stan grammar or catalog
rules. Full analysis is recomputed per accepted document revision; incremental
analysis is intentionally deferred until benchmarks demonstrate a need.

stanc3 remains the semantic authority. Local analysis favors responsive and
conservative feedback and does not report uncertain type errors.
