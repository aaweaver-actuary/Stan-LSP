use std::{collections::HashMap, sync::Arc};

use super::{AnalysisSnapshot, FileId, Revision, analyze_revision::analyze_revision};

#[derive(Debug, Default)]
/// Owns source text and the most recent fully recomputed snapshot for each file.
pub struct AnalysisHost {
    /// Monotonically increasing file ID used to assign stable identities to new files.
    next_file: u32,
    /// Maps file IDs to the most recent revision, source text, and snapshot.
    files: HashMap<FileId, (Revision, Arc<str>, Arc<AnalysisSnapshot>)>,
}

impl AnalysisHost {
    /// Adds a new source file at revision zero and returns its stable ID.
    pub fn add_file(&mut self, text: impl Into<Arc<str>>) -> FileId {
        let file = FileId(self.next_file);
        self.next_file += 1;
        self.set_file(file, Revision(0), text);
        file
    }

    /// Replaces a file and recomputes its entire analysis for `revision`.
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

    /// Returns the latest immutable snapshot for `file`.
    pub fn snapshot(&self, file: FileId) -> Option<Arc<AnalysisSnapshot>> {
        self.files
            .get(&file)
            .map(|(_, _, snapshot)| snapshot.clone())
    }

    /// Returns the source text associated with `file`.
    pub fn text(&self, file: FileId) -> Option<&str> {
        self.files.get(&file).map(|(_, text, _)| text.as_ref())
    }
}
