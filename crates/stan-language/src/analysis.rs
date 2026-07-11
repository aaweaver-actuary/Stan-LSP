//! Immutable analysis snapshots shared by editor features.

use std::{collections::HashMap, sync::Arc};

use crate::{
    Diagnostic, LexicalDiagnostic, SemanticModel, SyntaxTree, Token, analyze_semantics, lex, parse,
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
    AnalysisSnapshot {
        revision,
        tokens,
        syntax: parsed.tree,
        semantics,
        lexical_diagnostics: result.diagnostics.into(),
        diagnostics: diagnostics.into(),
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
