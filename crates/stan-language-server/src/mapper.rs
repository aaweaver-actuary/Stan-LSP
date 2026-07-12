//! The single UTF-8/UTF-16 conversion boundary used by LSP adapters.

use stan_language::{TextRange, TextSize};
use tower_lsp_server::ls_types::{Position, Range};

use crate::{
    document::Document,
    line_index::{LineIndex, PositionError},
};

#[derive(Debug, Clone, Copy)]
/// Authoritative conversion boundary between language byte ranges and LSP UTF-16.
pub struct LspMapper<'a> {
    text: &'a str,
    line_index: &'a LineIndex,
}

impl<'a> LspMapper<'a> {
    /// Borrows the text and line index from a cached document.
    pub fn for_document(document: &'a Document) -> Self {
        Self::new(&document.text, &document.line_index)
    }

    /// Constructs a mapper from matching text and line-index values.
    pub const fn new(text: &'a str, line_index: &'a LineIndex) -> Self {
        Self { text, line_index }
    }

    /// Converts a UTF-16 LSP position to a UTF-8 byte offset.
    pub fn to_offset(self, position: Position) -> Result<TextSize, PositionError> {
        let offset = self.line_index.position_to_offset(self.text, position)?;
        u32::try_from(offset).map_err(|_| PositionError::OffsetOutOfBounds)
    }

    /// Converts a UTF-8 byte offset to a UTF-16 LSP position.
    pub fn to_position(self, offset: TextSize) -> Result<Position, PositionError> {
        self.line_index
            .offset_to_position(self.text, offset as usize)
    }

    /// Converts a half-open UTF-8 byte range to an LSP range.
    pub fn to_range(self, range: TextRange) -> Result<Range, PositionError> {
        Ok(Range::new(
            self.to_position(range.start)?,
            self.to_position(range.end)?,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tower_lsp_server::ls_types::Uri;

    #[test]
    fn maps_unicode_ranges_in_both_directions() {
        let uri: Uri = "file:///unicode.stan".parse().unwrap();
        let document = Document::new(uri, 1, "// 😀\nmodel {}".to_owned());
        let mapper = LspMapper::for_document(&document);
        assert_eq!(mapper.to_offset(Position::new(0, 5)), Ok(7));
        assert_eq!(mapper.to_position(7), Ok(Position::new(0, 5)));
        assert_eq!(
            mapper.to_range(TextRange::new(7, 12)),
            Ok(Range::new(Position::new(0, 5), Position::new(1, 4)))
        );
    }
}
