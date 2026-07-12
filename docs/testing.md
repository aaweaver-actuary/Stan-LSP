# Testing strategy

The project uses five complementary layers:

1. Rust doc tests demonstrate deliberate public APIs.
2. Unit tests isolate lexing, parsing, scope lookup, range conversion, linting,
   formatting, compiler-output parsing, and invariants.
3. `.stan` fixtures with normalized `.expected.json` sidecars verify language and
   feature behavior.
4. Stdio protocol tests verify framing, capabilities, exact LSP results, Unicode,
   document versions, and shutdown behavior.
5. Corpus checks detect panics, invalid ranges, non-idempotent formatting, stanc3
   incompatibility, false local errors, and performance regressions.

Tests compare ordered normalized output. They must not depend on hash-map order.
Every fixed defect receives a focused regression test. Normal pull-request tests
use the checked-in smoke corpus; scheduled and release jobs fetch the pinned
external corpus and matching stanc3 compiler.
