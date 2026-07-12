# Diagnostic and lint codes

Published diagnostic codes are stable public identifiers within a minor release
series. New codes may be added in a patch release. A renamed or removed code keeps
a compatibility alias until the next minor release and is recorded in release
notes. Experimental rules are identified below and may change only at a minor
release boundary.

Local language diagnostics use `lex.`, `syntax.`, or `semantic.` prefixes and the
`stan-lsp` source. Configurable rules use their lint ID and the `stanlint` source.
Include diagnostics use the `stan-workspace` source. Compiler diagnostics use the
`stanc3` source.

| Code | Default | Meaning |
| --- | --- | --- |
| `lex.invalid-character` | error | Character is not part of Stan syntax. |
| `lex.unterminated-string` | error | String reaches a newline or EOF without closing. |
| `lex.unterminated-block-comment` | error | Block comment reaches EOF without closing. |
| `lex.malformed-number` | error | Numeric literal has invalid separators or exponent. |
| `lex.invalid-directive` | error | Unknown preprocessor directive. |
| `syntax.unmatched-closing-delimiter` | error | Closing delimiter has no matching opener. |
| `syntax.unclosed-delimiter` | error | Opening delimiter has no matching closer. |
| `syntax.duplicate-program-block` | error | A program block occurs more than once. |
| `syntax.missing-semicolon` | error | A statement is missing its terminating semicolon. |
| `syntax.invalid-program-block-header` | error | A compound block header is malformed. |
| `deprecated.language-element` | warning | A legacy Stan spelling or construct is used. |
| `syntax.program-block-order` | error | A program block appears after a block that must follow it. |
| `syntax.malformed-declaration` | error | A structurally complete declaration has no name. |
| `syntax.expected-expression` | error | A complete statement contains an invalid expression. |
| `semantic.duplicate-declaration` | error | A verified scope contains duplicate declarations. |
| `correctness.unresolved-identifier` | hint, experimental | A name cannot be resolved by the conservative local model. |
| `correctness.illegal-call-context` | hint, experimental | A restricted function appears in an incompatible context. |
| `correctness.argument-count` | error | No known overload accepts the argument count. |
| `correctness.argument-type` | hint, experimental | Completely known argument types match no overload. |
| `correctness.unknown-distribution` | hint, experimental | Complete sampling notation names no built-in or user probability function. |
| `suspicious.unused-declaration` | hint, experimental | A declaration has no resolved references. |
| `performance.repeated-expensive-operation` | hint, experimental | Expensive matrix work is repeated inside a loop. |
| `performance.vectorization-opportunity` | hint, experimental | A sampling loop may admit vectorization. |
| `bayesian.parameter-without-apparent-prior` | allow, experimental | Opt-in heuristic for parameters without an apparent prior contribution. |
| `workspace.invalid-include` | error | An include directive has no usable path. |
| `workspace.missing-include` | error | An include cannot be resolved from configured roots. |
| `workspace.include-cycle` | error | Workspace include dependencies form a cycle. |
| `stanc3.compiler` | error | Authoritative diagnostic emitted by stanc3. |
| `stanc3.version-mismatch` | information | Embedded catalog and compiler versions differ. |
| `internal.analysis-panic` | error | Analysis was isolated after an internal failure; please report the model and logs. |

Bayesian heuristic lints will remain disabled by default until their precision
has been established on representative projects.

Call-context, arity, type, and distribution findings are suppressed when the
corresponding syntax is incomplete. stanc3 remains authoritative when local type
or scope information is unknown.

`stanlint.toml` accepts stable IDs as keys and `"allow"`, `"hint"`, `"warn"`,
or `"deny"` values. A source comment such as
`// stanlint: allow correctness.unresolved-identifier` suppresses that lint on
the following line.
