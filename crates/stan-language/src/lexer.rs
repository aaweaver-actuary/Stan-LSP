//! Lossless lexical analysis for incomplete Stan source files.

use crate::{Directive, Keyword, LegacyLanguageElement, Symbol};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TextRange {
    pub start: u32,
    pub end: u32,
}

impl TextRange {
    pub const fn new(start: usize, end: usize) -> Self {
        Self {
            start: start as u32,
            end: end as u32,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SyntaxKind {
    Keyword(Keyword),
    Symbol(Symbol),
    Directive(Directive),
    IncludePath,
    Legacy(LegacyLanguageElement),
    Identifier,
    IntegerLiteral,
    RealLiteral,
    ImaginaryLiteral,
    StringLiteral,
    LineComment,
    BlockComment,
    Whitespace,
    Invalid,
    EndOfFile,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Token {
    pub kind: SyntaxKind,
    pub range: TextRange,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LexicalDiagnosticKind {
    InvalidCharacter,
    UnterminatedString,
    UnterminatedBlockComment,
    MalformedNumber,
    InvalidDirective,
}

impl LexicalDiagnosticKind {
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidCharacter => "lex.invalid-character",
            Self::UnterminatedString => "lex.unterminated-string",
            Self::UnterminatedBlockComment => "lex.unterminated-block-comment",
            Self::MalformedNumber => "lex.malformed-number",
            Self::InvalidDirective => "lex.invalid-directive",
        }
    }

    pub const fn message(self) -> &'static str {
        match self {
            Self::InvalidCharacter => "invalid character in Stan source",
            Self::UnterminatedString => "unterminated string literal",
            Self::UnterminatedBlockComment => "unterminated block comment",
            Self::MalformedNumber => "malformed numeric literal",
            Self::InvalidDirective => "invalid Stan preprocessor directive",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LexicalDiagnostic {
    pub kind: LexicalDiagnosticKind,
    pub range: TextRange,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LexResult {
    pub tokens: Vec<Token>,
    pub diagnostics: Vec<LexicalDiagnostic>,
}

pub fn lex(text: &str) -> LexResult {
    Lexer {
        text,
        offset: 0,
        expect_include_path: false,
        tokens: Vec::new(),
        diagnostics: Vec::new(),
    }
    .run()
}

struct Lexer<'a> {
    text: &'a str,
    offset: usize,
    tokens: Vec<Token>,
    diagnostics: Vec<LexicalDiagnostic>,
    expect_include_path: bool,
}

impl Lexer<'_> {
    fn run(mut self) -> LexResult {
        while self.offset < self.text.len() {
            self.next_token();
        }
        self.tokens.push(Token {
            kind: SyntaxKind::EndOfFile,
            range: TextRange::new(self.offset, self.offset),
        });
        LexResult {
            tokens: self.tokens,
            diagnostics: self.diagnostics,
        }
    }

    fn next_token(&mut self) {
        let start = self.offset;
        let remaining = &self.text[start..];
        let first = remaining.chars().next().expect("source remains");

        if first.is_whitespace() {
            self.consume_while(char::is_whitespace);
            if self.expect_include_path
                && self.text[start..self.offset]
                    .chars()
                    .any(|character| character == '\n' || character == '\r')
            {
                self.expect_include_path = false;
            }
            return self.push(SyntaxKind::Whitespace, start);
        }
        if self.expect_include_path {
            self.expect_include_path = false;
            if first == '"' {
                self.offset += first.len_utf8();
                while self.offset < self.text.len() {
                    let character = self.text[self.offset..]
                        .chars()
                        .next()
                        .expect("source remains");
                    self.offset += character.len_utf8();
                    if character == '"' {
                        break;
                    }
                }
            } else {
                self.consume_while(|character| !character.is_whitespace());
            }
            return self.push(SyntaxKind::IncludePath, start);
        }
        if remaining.starts_with("//") {
            self.offset += 2;
            self.consume_while(|character| character != '\n' && character != '\r');
            return self.push(SyntaxKind::LineComment, start);
        }
        if remaining.starts_with("/*") {
            self.offset += 2;
            if let Some(end) = self.text[self.offset..].find("*/") {
                self.offset += end + 2;
            } else {
                self.offset = self.text.len();
                self.diagnostic(LexicalDiagnosticKind::UnterminatedBlockComment, start);
            }
            return self.push(SyntaxKind::BlockComment, start);
        }
        if first == '"' {
            return self.string(start);
        }
        if first == '#' {
            return self.directive(start);
        }
        if first.is_ascii_digit()
            || (first == '.'
                && remaining
                    .chars()
                    .nth(1)
                    .is_some_and(|character| character.is_ascii_digit()))
        {
            return self.number(start);
        }
        if first == '_' || first.is_alphabetic() {
            self.consume_while(|character| character == '_' || character.is_alphanumeric());
            let spelling = &self.text[start..self.offset];
            let kind = Keyword::from_str(spelling)
                .map(SyntaxKind::Keyword)
                .or_else(|_| LegacyLanguageElement::from_str(spelling).map(SyntaxKind::Legacy))
                .unwrap_or(SyntaxKind::Identifier);
            return self.push(kind, start);
        }
        if let Some(legacy) = LegacyLanguageElement::ALL
            .iter()
            .filter(|element| self.text[start..].starts_with(element.as_str()))
            .max_by_key(|element| element.as_str().len())
        {
            self.offset += legacy.as_str().len();
            return self.push(SyntaxKind::Legacy(*legacy), start);
        }
        if let Some(symbol) = Symbol::ALL
            .iter()
            .filter(|symbol| self.text[start..].starts_with(symbol.as_str()))
            .max_by_key(|symbol| symbol.as_str().len())
        {
            self.offset += symbol.as_str().len();
            return self.push(SyntaxKind::Symbol(*symbol), start);
        }

        self.offset += first.len_utf8();
        self.diagnostic(LexicalDiagnosticKind::InvalidCharacter, start);
        self.push(SyntaxKind::Invalid, start);
    }

    fn string(&mut self, start: usize) {
        self.offset += 1;
        let mut escaped = false;
        while self.offset < self.text.len() {
            let character = self.text[self.offset..]
                .chars()
                .next()
                .expect("source remains");
            self.offset += character.len_utf8();
            if escaped {
                escaped = false;
            } else if character == '\\' {
                escaped = true;
            } else if character == '"' {
                return self.push(SyntaxKind::StringLiteral, start);
            } else if character == '\n' || character == '\r' {
                self.diagnostic(LexicalDiagnosticKind::UnterminatedString, start);
                return self.push(SyntaxKind::StringLiteral, start);
            }
        }
        self.diagnostic(LexicalDiagnosticKind::UnterminatedString, start);
        self.push(SyntaxKind::StringLiteral, start);
    }

    fn directive(&mut self, start: usize) {
        if self.text[start..].starts_with(Directive::Include.as_str()) {
            self.offset += Directive::Include.as_str().len();
            self.expect_include_path = true;
            self.push(SyntaxKind::Directive(Directive::Include), start);
        } else {
            self.offset += 1;
            self.consume_while(|character| character == '_' || character.is_alphanumeric());
            self.diagnostic(LexicalDiagnosticKind::InvalidDirective, start);
            self.push(SyntaxKind::Invalid, start);
        }
    }

    fn number(&mut self, start: usize) {
        let mut real = false;
        let mut malformed = false;
        if self.text[self.offset..].starts_with('.') {
            real = true;
            self.offset += 1;
        }
        malformed |= !self.digits_with_underscores();
        if self.text[self.offset..].starts_with('.') {
            real = true;
            self.offset += 1;
            if self.text[self.offset..].starts_with('_') {
                malformed = true;
            }
            let _ = self.digits_with_underscores();
        }
        if self.text[self.offset..].starts_with(['e', 'E']) {
            real = true;
            self.offset += 1;
            if self.text[self.offset..].starts_with(['+', '-']) {
                self.offset += 1;
            }
            if !self.digits_with_underscores() {
                malformed = true;
            }
        }
        let imaginary = self.text[self.offset..].starts_with('i');
        if imaginary {
            self.offset += 1;
        }
        if malformed {
            self.diagnostic(LexicalDiagnosticKind::MalformedNumber, start);
        }
        self.push(
            if imaginary {
                SyntaxKind::ImaginaryLiteral
            } else if real {
                SyntaxKind::RealLiteral
            } else {
                SyntaxKind::IntegerLiteral
            },
            start,
        );
    }

    fn digits_with_underscores(&mut self) -> bool {
        let mut saw_digit = false;
        let mut previous_underscore = false;
        let mut valid = true;
        while let Some(character) = self.text[self.offset..].chars().next() {
            if character.is_ascii_digit() {
                saw_digit = true;
                previous_underscore = false;
                self.offset += 1;
            } else if character == '_' {
                if !saw_digit || previous_underscore {
                    valid = false;
                }
                previous_underscore = true;
                self.offset += 1;
            } else {
                break;
            }
        }
        saw_digit && valid && !previous_underscore
    }

    fn consume_while(&mut self, predicate: impl Fn(char) -> bool) {
        while let Some(character) = self.text[self.offset..].chars().next() {
            if !predicate(character) {
                break;
            }
            self.offset += character.len_utf8();
        }
    }

    fn diagnostic(&mut self, kind: LexicalDiagnosticKind, start: usize) {
        self.diagnostics.push(LexicalDiagnostic {
            kind,
            range: TextRange::new(start, self.offset),
        });
    }

    fn push(&mut self, kind: SyntaxKind, start: usize) {
        self.tokens.push(Token {
            kind,
            range: TextRange::new(start, self.offset),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lexing_is_lossless_and_longest_match_wins() {
        let source = "model { y .*= 2.0; // note\n z = 1e-2i; }";
        let result = lex(source);
        assert!(result.diagnostics.is_empty());
        let reconstructed = result
            .tokens
            .iter()
            .filter(|token| token.kind != SyntaxKind::EndOfFile)
            .map(|token| &source[token.range.start as usize..token.range.end as usize])
            .collect::<String>();
        assert_eq!(reconstructed, source);
        assert!(
            result
                .tokens
                .iter()
                .any(|token| { token.kind == SyntaxKind::Symbol(Symbol::ElementwiseTimesAssign) })
        );
    }

    #[test]
    fn reports_a_stable_invalid_character_diagnostic() {
        let result = lex("model { @ }");
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(
            result.diagnostics[0].kind,
            LexicalDiagnosticKind::InvalidCharacter
        );
        assert_eq!(result.diagnostics[0].range, TextRange::new(8, 9));
    }

    #[test]
    fn reports_unterminated_constructs() {
        assert_eq!(
            lex("/* unfinished").diagnostics[0].kind,
            LexicalDiagnosticKind::UnterminatedBlockComment
        );
        assert_eq!(
            lex("\"unfinished").diagnostics[0].kind,
            LexicalDiagnosticKind::UnterminatedString
        );
    }

    #[test]
    fn every_symbol_uses_longest_match_and_round_trips() {
        for symbol in Symbol::ALL {
            let result = lex(symbol.as_str());
            assert_eq!(result.tokens[0].kind, SyntaxKind::Symbol(*symbol));
            assert_eq!(
                result.tokens[0].range,
                TextRange::new(0, symbol.as_str().len())
            );
        }
    }

    #[test]
    fn malformed_numbers_are_diagnosed_conservatively() {
        assert!(lex("1_000 2.5 1e-3 4i").diagnostics.is_empty());
        for source in ["1_", "1__2", "1e", "1e+"] {
            assert_eq!(
                lex(source).diagnostics[0].kind,
                LexicalDiagnosticKind::MalformedNumber,
                "{source}"
            );
        }
    }

    #[test]
    fn include_paths_are_single_lossless_tokens() {
        let result = lex("#include \"shared/functions.stan\"\n");
        assert!(result.diagnostics.is_empty());
        assert!(
            result
                .tokens
                .iter()
                .any(|token| token.kind == SyntaxKind::IncludePath)
        );
    }
}
