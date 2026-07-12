# Native formatting

`stanfmt` formats the lossless syntax representation and preserves comments,
directives, includes, strings, and source meaning. Formatting is deterministic and
idempotent. Range formatting expands to a complete syntax construct.

The formatter is experimental until the pinned corpus establishes zero panics,
zero non-idempotence, and zero stanc-valid models made invalid after formatting.
Malformed input returns a controlled error or receives only demonstrably safe
edits. stanc formatting is a differential oracle, not the runtime backend.
