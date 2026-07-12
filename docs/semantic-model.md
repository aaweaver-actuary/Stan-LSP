# Semantic model

The local semantic model is a per-file, conservative index. It identifies program
declarations, functions, parameters, locals, loop binders, scopes, references,
selected inferred types, and user-defined probability functions.

Resolution searches the current lexical scope followed by its parents, observes
declaration order, and selects the nearest visible declaration. Function names are
hoisted within the functions block. Function bodies cannot capture program-block
variables. Data, transformed data, parameters, and transformed parameters follow
Stan program-block visibility; model and generated-quantities locals remain local.

Unknown or incomplete constructs remain unresolved instead of producing guessed
bindings. Built-in types and stanc3 are authoritative where local inference is
incomplete. `SemanticModel::validate` checks IDs, parents, containment, binder
scopes, and resolved references in tests and debug builds.
