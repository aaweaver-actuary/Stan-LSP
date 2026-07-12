//! Versioned open-document state and cached immutable analysis snapshots.

use std::collections::HashMap;

use stan_language::{AnalysisSnapshot, Revision, analyze_revision, fallback_analysis};
use std::sync::Arc;
use tower_lsp_server::ls_types::Uri;

use crate::line_index::LineIndex;

#[derive(Debug, Clone)]
/// One accepted open-document revision and its shared analysis.
pub struct Document {
    /// Canonical LSP document URI.
    pub uri: Uri,
    /// Latest accepted client version.
    pub version: i32,
    /// Complete UTF-8 source text.
    pub text: String,
    /// Cached UTF-16 conversion index.
    pub line_index: LineIndex,
    /// Analysis computed exactly once for this revision.
    pub analysis: Arc<AnalysisSnapshot>,
}

impl Document {
    /// Creates and fully analyzes one document revision, isolating internal panics.
    pub fn new(uri: Uri, version: i32, text: String) -> Self {
        let line_index = LineIndex::new(&text);
        let revision = Revision(version.max(0) as u64);
        let analysis = Arc::new(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                analyze_revision(&text, revision)
            }))
            .unwrap_or_else(|_| {
                tracing::error!(uri = %uri.as_str(), version, "Stan analysis panicked");
                fallback_analysis(&text, revision, "internal analysis failure")
            }),
        );
        Self {
            uri,
            version,
            text,
            line_index,
            analysis,
        }
    }
}

#[derive(Debug, Default)]
/// URI-keyed store that rejects stale full-document updates.
pub struct DocumentStore {
    documents: HashMap<Uri, Document>,
}

impl DocumentStore {
    /// Inserts or replaces an open document.
    pub fn open(&mut self, document: Document) {
        self.documents.insert(document.uri.clone(), document);
    }

    /// Accepts a strictly newer full-document version.
    ///
    /// Returns `None` for unknown documents and stale versions.
    pub fn replace(&mut self, uri: &Uri, version: i32, text: String) -> Option<&Document> {
        let document = self.documents.get_mut(uri)?;
        if version <= document.version {
            return None;
        }
        *document = Document::new(uri.clone(), version, text);
        Some(document)
    }

    /// Removes and returns a closed document.
    pub fn close(&mut self, uri: &Uri) -> Option<Document> {
        self.documents.remove(uri)
    }

    /// Returns the latest accepted document revision.
    pub fn get(&self, uri: &Uri) -> Option<&Document> {
        self.documents.get(uri)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stale_versions_do_not_replace_documents() {
        let uri: Uri = "file:///model.stan".parse().unwrap();
        let mut store = DocumentStore::default();
        store.open(Document::new(uri.clone(), 2, "model {}".to_owned()));
        assert!(store.replace(&uri, 1, "@".to_owned()).is_none());
        let document = store.replace(&uri, 3, "data {}".to_owned()).unwrap();
        assert_eq!(document.version, 3);
        assert_eq!(document.text, "data {}");
    }
}
