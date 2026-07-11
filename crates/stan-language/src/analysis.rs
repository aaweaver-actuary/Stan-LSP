//! Immutable analysis snapshots shared by editor features.

use std::{
    collections::{BTreeMap, HashMap},
    sync::Arc,
};

use crate::{
    Diagnostic, LexicalDiagnostic, SemanticModel, StanType, SyntaxKind, SyntaxTree, TextRange,
    Token, analyze_semantics, lex, parse,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FileId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Revision(pub u64);

#[derive(Debug, Clone)]
pub struct AnalysisSnapshot {
    pub revision: Revision,
    pub tokens: Arc<[Token]>,
    pub syntax: SyntaxTree,
    pub semantics: SemanticModel,
    pub lexical_diagnostics: Arc<[LexicalDiagnostic]>,
    pub diagnostics: Arc<[Diagnostic]>,
    pub inferred_types: Arc<BTreeMap<TextRange, StanType>>,
    pub include_dependencies: Arc<[String]>,
}

pub type Analysis = AnalysisSnapshot;

/// Recomputes the complete local analysis for a document.
///
/// Future syntax and semantic stages can be added to this snapshot without
/// coupling them to the language-server transport.
pub fn analyze(text: &str) -> AnalysisSnapshot {
    analyze_revision(text, Revision::default())
}

pub fn analyze_revision(text: &str, revision: Revision) -> AnalysisSnapshot {
    let result = lex(text);
    let tokens: Arc<[Token]> = result.tokens.into();
    let parsed = parse(Arc::<str>::from(text), tokens.clone());
    let semantics = analyze_semantics(text, &tokens, &parsed.tree);
    let include_dependencies = tokens
        .iter()
        .filter(|token| token.kind == SyntaxKind::IncludePath)
        .map(|token| {
            text[token.range.start as usize..token.range.end as usize]
                .trim_matches('"')
                .to_owned()
        })
        .collect::<Vec<_>>();
    let diagnostics = result
        .diagnostics
        .iter()
        .map(|diagnostic| {
            Diagnostic::error(
                diagnostic.kind.code(),
                diagnostic.kind.message(),
                diagnostic.range,
            )
        })
        .chain(parsed.diagnostics)
        .chain(semantics.diagnostics.iter().cloned())
        .collect::<Vec<_>>();
    let inferred_types = Arc::new(semantics.inferred_types.clone());
    AnalysisSnapshot {
        revision,
        tokens,
        syntax: parsed.tree,
        semantics,
        lexical_diagnostics: result.diagnostics.into(),
        diagnostics: diagnostics.into(),
        inferred_types,
        include_dependencies: include_dependencies.into(),
    }
}

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

#[derive(Debug, Default)]
pub struct AnalysisHost {
    next_file: u32,
    files: HashMap<FileId, (Revision, Arc<str>, Arc<AnalysisSnapshot>)>,
}

impl AnalysisHost {
    pub fn add_file(&mut self, text: impl Into<Arc<str>>) -> FileId {
        let file = FileId(self.next_file);
        self.next_file += 1;
        self.set_file(file, Revision(0), text);
        file
    }

    pub fn set_file(
        &mut self,
        file: FileId,
        revision: Revision,
        text: impl Into<Arc<str>>,
    ) -> Arc<AnalysisSnapshot> {
        let text = text.into();
        let snapshot = Arc::new(analyze_revision(&text, revision));
        self.files.insert(file, (revision, text, snapshot.clone()));
        snapshot
    }

    pub fn snapshot(&self, file: FileId) -> Option<Arc<AnalysisSnapshot>> {
        self.files
            .get(&file)
            .map(|(_, _, snapshot)| snapshot.clone())
    }

    pub fn text(&self, file: FileId) -> Option<&str> {
        self.files.get(&file).map(|(_, text, _)| text.as_ref())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
