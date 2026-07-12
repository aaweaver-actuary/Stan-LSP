# Parser wrapper contracts

The parser produces a flat, lossless recovery tree. Typed wrappers borrow that
tree and expose only syntax that can be identified without guessing. All ranges
are half-open UTF-8 byte offsets, and all iterators preserve source order.

An incomplete construct may still have a wrapper. Optional accessors return
`None` for missing syntax, while child iterators omit fragments that lack the
minimum structure required by the child wrapper. Parsing and accessor calls must
not panic for arbitrary editor input.

## Coverage matrix

| Wrapper | Semantic consumer | Editor consumer | Valid fixture | Recovery fixture | Exact ranges |
| --- | --- | --- | --- | --- | --- |
| `SourceFile` | declaration and scope extraction | indirect through analysis | `program-blocks/valid` | all recovery fixtures | source, tokens, children |
| `ProgramBlock` | block visibility | symbols and folding | `program-blocks/valid` | `recovery/loop` | kind and block range |
| `CompoundStatement` | scope construction | indirect through semantics | `expressions/valid` | all recovery fixtures | brace range and containment |
| `FunctionDeclaration` | functions and parameters | navigation through semantics | `function-declarations/valid` | `recovery/function` | declaration, name, return, body |
| `FunctionParameter` | parameter symbols and types | local language features | `function-declarations/valid` | `recovery/function` | parameter, name, type |
| `VariableDeclaration` | variable symbols and types | local language features | `variable-declarations/valid` | `recovery/declaration` | declaration and type |
| `Declarator` | individual variable symbols | local language features | `variable-declarations/valid` | `recovery/declaration` | declarator and name |
| `ForStatement` | loop scope and binder | navigation through semantics | `expressions/valid` | `recovery/loop` | loop, binder, body |
| `CallExpression` | call linting | catalog-backed assistance | `expressions/valid` | `recovery/sampling` | call, callee, close delimiter |
| `SamplingStatement` | probability relationships | distribution assistance | `expressions/valid` | `recovery/sampling` | statement, distribution, close delimiter |
| `TypeSyntax` | declared-type parsing | hover and symbols through semantics | function and variable fixtures | function and declaration recovery | spelling and range |
| `NameRef` | declaration and binder extraction | navigation through semantics | every construct fixture | function, loop, and sampling recovery | spelling and range |

Fixture sidecars contain normalized structured output. Numeric ranges are
explicit so a change in recovery boundaries or token ownership requires review.
The harness also checks reconstruction, source bounds, parent containment, token
monotonicity, and deterministic wrapper ordering.

## Current limitations

- The tree is flat: typed nodes normally have the source file as their untyped
  parent. Typed relationships are computed from token ranges.
- Parameters and declarators without both a recognizable type and name are
  omitted instead of represented as invalid typed children.
- Call argument and sampling-outcome ranges do not have dedicated public
  wrappers. Tests validate the currently published contracts.
- Recovery recognizes only unambiguous grammar roles and never synthesizes
  identifiers, types, delimiters, or bodies.

Add a focused fixture and sidecar whenever a wrapper contract changes or a
parser defect is fixed. Unsupported grammar belongs in a separate issue.
