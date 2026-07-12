# Adding an editor feature

Before implementing a feature, document its syntax and semantic prerequisites and
whether its result is stable or experimental. Put language behavior in
`stan-language` or a protocol-independent server feature module. LSP handlers
should only retrieve state, map positions, invoke behavior, and translate results.

Every feature change requires:

- focused language or feature unit tests;
- valid and malformed fixtures;
- exact protocol ranges and payload assertions;
- Unicode and stale-revision coverage when applicable;
- diagnostic/configuration documentation changes;
- a repository-map update if responsibilities move;
- corpus results for parser, semantic, lint, or formatter changes.
