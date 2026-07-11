use tower_lsp_server::ls_types::Position;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PositionError {
    LineOutOfBounds,
    CharacterOutOfBounds,
    OffsetOutOfBounds,
    NotCharacterBoundary,
    InsideUtf16Character,
}

#[derive(Debug, Clone)]
pub struct LineIndex {
    line_starts: Vec<usize>,
}

impl LineIndex {
    pub fn new(text: &str) -> Self {
        let mut line_starts = vec![0];
        for (offset, byte) in text.bytes().enumerate() {
            if byte == b'\n' {
                line_starts.push(offset + 1);
            }
        }
        Self { line_starts }
    }

    pub fn position_to_offset(
        &self,
        text: &str,
        position: Position,
    ) -> Result<usize, PositionError> {
        let line = position.line as usize;
        let start = *self
            .line_starts
            .get(line)
            .ok_or(PositionError::LineOutOfBounds)?;
        let end = self
            .line_starts
            .get(line + 1)
            .copied()
            .unwrap_or(text.len());
        let content_end = line_content_end(text, start, end);
        let mut utf16 = 0u32;
        for (relative, character) in text[start..content_end].char_indices() {
            if utf16 == position.character {
                return Ok(start + relative);
            }
            utf16 += character.len_utf16() as u32;
            if utf16 > position.character {
                return Err(PositionError::InsideUtf16Character);
            }
        }
        if utf16 == position.character {
            Ok(content_end)
        } else {
            Err(PositionError::CharacterOutOfBounds)
        }
    }

    pub fn offset_to_position(&self, text: &str, offset: usize) -> Result<Position, PositionError> {
        if offset > text.len() {
            return Err(PositionError::OffsetOutOfBounds);
        }
        if !text.is_char_boundary(offset) {
            return Err(PositionError::NotCharacterBoundary);
        }
        let line = self.line_starts.partition_point(|start| *start <= offset) - 1;
        let start = self.line_starts[line];
        let character = text[start..offset]
            .chars()
            .map(|character| character.len_utf16() as u32)
            .sum();
        Ok(Position::new(line as u32, character))
    }
}

fn line_content_end(text: &str, start: usize, end: usize) -> usize {
    let mut content_end = end;
    if content_end > start && text.as_bytes()[content_end - 1] == b'\n' {
        content_end -= 1;
    }
    if content_end > start && text.as_bytes()[content_end - 1] == b'\r' {
        content_end -= 1;
    }
    content_end
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_utf16_positions_and_offsets() {
        let text = "a😀b\r\nlast\n";
        let index = LineIndex::new(text);
        assert_eq!(index.offset_to_position(text, 5), Ok(Position::new(0, 3)));
        assert_eq!(index.position_to_offset(text, Position::new(0, 3)), Ok(5));
        assert_eq!(
            index.position_to_offset(text, Position::new(0, 2)),
            Err(PositionError::InsideUtf16Character)
        );
        assert_eq!(index.position_to_offset(text, Position::new(1, 4)), Ok(12));
        assert_eq!(index.position_to_offset(text, Position::new(2, 0)), Ok(13));
    }
}
