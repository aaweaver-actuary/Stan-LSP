# Diagnostic and lint codes

Diagnostic codes are stable public identifiers. Local language diagnostics use
the `lex.` or `syntax.` prefix, stanc3 diagnostics use `stanc3.`, and configurable
lint rules use their lint ID.

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
| `correctness.unresolved-identifier` | error | A name cannot be resolved locally or as a built-in. |
| `correctness.illegal-call-context` | error | A restricted function is called in an illegal block. |
| `correctness.argument-count` | error | No known overload accepts the argument count. |
| `correctness.argument-type` | error | Known argument types match no overload. |
| `suspicious.unused-declaration` | warning | A declaration has no references. |
| `bayesian.parameter-without-apparent-prior` | allow | Opt-in heuristic for parameters without an apparent prior contribution. |
| `stanc3.compiler` | error | Authoritative diagnostic emitted by stanc3. |
| `stanc3.version-mismatch` | information | Embedded catalog and compiler versions differ. |

Bayesian heuristic lints will remain disabled by default until their precision
has been established on representative projects.

`stanlint.toml` accepts stable IDs as keys and `"allow"`, `"hint"`, `"warn"`,
or `"deny"` values. A source comment such as
`// stanlint: allow correctness.unresolved-identifier` suppresses that lint on
the following line.
