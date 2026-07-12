# stanc3 integration

Compiler discovery uses explicit configuration, `STANC`, `PATH`, and reliable
CmdStan locations in that order. The server records the compiler version and emits
an informational mismatch diagnostic when it differs from the embedded Stan 2.39
catalog.

Compiler work is asynchronous, cancellable, bounded by a timeout, and associated
with a document revision. Unsaved buffers are materialized beside the source when
necessary so relative includes retain their meaning. Results from stale revisions
are discarded. Compiler diagnostics use source `stanc3`; local analysis does not
claim to replace them.
