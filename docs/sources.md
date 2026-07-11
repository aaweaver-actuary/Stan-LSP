# Stan 2.39 catalog provenance

The embedded catalog is pinned to **Stan 2.39.0**. The source manuals supplied for
the initial classification work were:

- *Stan Reference Manual*, version 2.39, Stan Development Team.
- *Stan Functions Reference*, version 2.39, Stan Development Team.
- *Stan User's Guide*, version 2.39, Stan Development Team.

The PDFs are not copied into this repository. Stan documentation text is published
under CC-BY-ND 4.0; only factual token spellings, type signatures, classifications,
and provenance are represented here.

## Authoritative sources

- [stanc3 v2.39.0 release](https://github.com/stan-dev/stanc3/releases/tag/v2.39.0)
- [stanc3 v2.39.0 lexer](https://github.com/stan-dev/stanc3/blob/v2.39.0/src/frontend/lexer.mll)
- [stanc3 signature generator](https://github.com/stan-dev/stanc3/blob/v2.39.0/src/stan_math_signatures/Generate.ml)
- [Developer documentation for exposed functions](https://mc-stan.org/stanc3/stanc/exposing_new_functions.html)

The fixed-language inventory comes from the reference manual BNF and the pinned
lexer. Concrete overloads come from:

```text
stanc3 v2.39.0 (Unix)
stanc --dump-stan-math-signatures
```

The original compiler dump contains 23,983 lines. The normalized repository
snapshot also includes compiler-supported unnormalized probability variants,
resulting in more than 28,000 typed concrete overloads. Higher-order functions
handled specially by the compiler are maintained in
`catalog/stan-2.39/special-signatures.tsv` using the function manual and compiler
behavior.

Documented introduction versions extracted from the manual are preserved in
`catalog/stan-2.39/availability.tsv`. Entries that could not be associated with a
function reliably remain explicitly unknown rather than being guessed.

The official macOS ARM release binary used for the first refresh had the upstream
SHA-256 digest:

```text
b0e26c50488bf73769f08d9ccff7d96781b31d09d6826f18fda827149170f199
```

## Classification policy

- The compiler is authoritative for accepted spellings and concrete overloads.
- The manuals are authoritative for semantic grouping, availability, deprecated
  and removed features, and user-facing signature templates.
- A function can belong to multiple categories when its overloads span scalar,
  array, matrix, or complex domains.
- Distribution statement names are separate from callable function names because
  the same spelling may be meaningful in both contexts.
- Removed elements remain discoverable but are unavailable in the 2.39 lifecycle
  model so future diagnostics can suggest migrations.

Run the refresh command with `--check` to verify that checked-in data and generated
enums have not drifted from an official 2.39 compiler dump.
