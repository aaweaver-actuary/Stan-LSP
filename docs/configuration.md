# Configuration

The server accepts these initialization options and matching `stanLsp.*` VS Code
settings:

| Setting | Default | Purpose |
| --- | --- | --- |
| `stanVersion` | `2.39.0` | Select an embedded catalog; unsupported versions fail initialization explicitly. |
| `stancPath` | `null` | Override `STANC` and `PATH` compiler discovery. |
| `includePaths` | `[]` | Additional include search roots. |
| `compilerOnSave` | `true` | Run authoritative stanc validation on save. |
| `compilerOnChange` | `false` | Run stanc after debounced document changes. |
| `compilerDebounceMs` | `500` | Delay before change-triggered compiler validation. |
| `formatIndentWidth` | `2` | Native formatter indentation width. |
| `lintLevels` | `{}` | Per-rule `allow`, `hint`, `warn`, or `deny` overrides. |

`stanfmt` reads `stanfmt.toml` from its working directory. The currently stable
option is `indent_width = 2`.

`stanlint` reads `stanlint.toml`. Keys are stable lint IDs documented in
[`diagnostics.md`](diagnostics.md), for example:

```toml
correctness.unresolved-identifier = "deny"
suspicious.unused-declaration = "warn"
bayesian.parameter-without-apparent-prior = "hint"
```
