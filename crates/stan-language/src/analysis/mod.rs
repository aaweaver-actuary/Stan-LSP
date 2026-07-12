//! Immutable analysis snapshots shared by editor features.

use std::{collections::BTreeMap, sync::Arc};

use crate::{Diagnostic, SemanticModel, SyntaxKind, TextRange, Token, parse};

pub mod analyze_revision;
pub mod host;
pub mod snapshot;

pub use analyze_revision::analyze_revision;
pub use host::AnalysisHost;
pub use snapshot::AnalysisSnapshot;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
/// Stable identity assigned to a source file by an [`AnalysisHost`].
pub struct FileId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
/// Monotonically increasing source revision used to reject stale results.
pub struct Revision(pub u64);

/// Constructs a minimal valid snapshot after the server isolates an internal failure.
pub fn fallback_analysis(
    text: &str,
    revision: Revision,
    message: impl Into<String>,
) -> AnalysisSnapshot {
    let tokens: Arc<[Token]> = vec![Token {
        kind: SyntaxKind::EndOfFile,
        range: TextRange::new(text.len(), text.len()),
    }]
    .into();
    let parsed = parse(Arc::<str>::from(text), tokens.clone());
    AnalysisSnapshot {
        revision,
        tokens,
        syntax: parsed.tree,
        semantics: SemanticModel::default(),
        lexical_diagnostics: Arc::from([]),
        diagnostics: vec![Diagnostic::error(
            "internal.analysis-panic",
            message,
            TextRange::new(0, 0),
        )]
        .into(),
        inferred_types: Arc::new(BTreeMap::new()),
        include_dependencies: Arc::from([]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::StanType;

    #[test]
    fn snapshots_are_revisioned_and_contain_cross_feature_data() {
        let mut host = AnalysisHost::default();
        let file = host.add_file("#include shared.stan\nmodel { real x; x = 1; }");
        let snapshot = host.snapshot(file).unwrap();
        assert_eq!(snapshot.revision, Revision(0));
        assert_eq!(snapshot.include_dependencies.as_ref(), &["shared.stan"]);
        assert!(
            snapshot
                .inferred_types
                .values()
                .any(|r#type| *r#type == StanType::Int)
        );
        let updated = host.set_file(file, Revision(1), "model {}");
        assert_eq!(updated.revision, Revision(1));
    }
}
