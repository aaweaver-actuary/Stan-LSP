use std::collections::HashMap;

use tower_lsp_server::ls_types::Uri;

use crate::line_index::LineIndex;

#[derive(Debug, Clone)]
pub struct Document {
    pub uri: Uri,
    pub version: i32,
    pub text: String,
    pub line_index: LineIndex,
}

impl Document {
    pub fn new(uri: Uri, version: i32, text: String) -> Self {
        let line_index = LineIndex::new(&text);
        Self {
            uri,
            version,
            text,
            line_index,
        }
    }
}

#[derive(Debug, Default)]
pub struct DocumentStore {
    documents: HashMap<Uri, Document>,
}

impl DocumentStore {
    pub fn open(&mut self, document: Document) {
        self.documents.insert(document.uri.clone(), document);
    }

    pub fn replace(&mut self, uri: &Uri, version: i32, text: String) -> Option<&Document> {
        let document = self.documents.get_mut(uri)?;
        if version <= document.version {
            return None;
        }
        *document = Document::new(uri.clone(), version, text);
        Some(document)
    }

    pub fn close(&mut self, uri: &Uri) -> Option<Document> {
        self.documents.remove(uri)
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
