# Adding a lint

Register each lint with a stable ID, group, default level, stability, and concise
description. State the confidence level, false-positive risks, required syntax and
semantic information, and behavior on incomplete source.

High-confidence correctness rules may be errors. Partial semantic checks default
to hints. Bayesian advice defaults to allow. A fix may be marked `Always` only when
it preserves source meaning for every matched input.

Add positive, negative, incomplete-syntax, suppression, configuration, and fix
tests. Document the code in `diagnostics.md` and record corpus false-positive
results before raising its default severity.
