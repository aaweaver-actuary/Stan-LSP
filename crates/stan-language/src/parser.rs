//! A lossless, recovery-oriented concrete syntax model.

use std::{collections::BTreeMap, sync::Arc};

use crate::{
    Applicability, Diagnostic, Fix, Keyword, ProgramBlockKind, RelatedDiagnostic, Symbol,
    SyntaxKind, TextEdit, TextRange, Token,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SyntaxNodeKind {
    SourceFile,
    ProgramBlock(ProgramBlockKind),
    Include,
    FunctionDeclaration,
    VariableDeclaration,
    Type,
    Constraint,
    ParameterList,
    CompoundStatement,
    Expression,
    ForStatement,
    FunctionCall,
    SamplingStatement,
    Statement,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyntaxNode {
    pub kind: SyntaxNodeKind,
    pub range: TextRange,
    pub token_range: std::ops::Range<usize>,
    pub parent: Option<usize>,
}

#[derive(Debug, Clone)]
pub struct SyntaxTree {
    text: Arc<str>,
    tokens: Arc<[Token]>,
    nodes: Arc<[SyntaxNode]>,
}

impl SyntaxTree {
    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn tokens(&self) -> &[Token] {
        &self.tokens
    }

    pub fn nodes(&self) -> &[SyntaxNode] {
        &self.nodes
    }

    pub fn source_file(&self) -> SourceFile<'_> {
        SourceFile { tree: self }
    }

    pub fn reconstruct(&self) -> String {
        self.tokens
            .iter()
            .filter(|token| token.kind != SyntaxKind::EndOfFile)
            .map(|token| &self.text[token.range.start as usize..token.range.end as usize])
            .collect()
    }
}

#[derive(Debug, Clone, Copy)]
pub struct SourceFile<'a> {
    tree: &'a SyntaxTree,
}

impl<'a> SourceFile<'a> {
    pub fn program_blocks(self) -> impl Iterator<Item = ProgramBlock<'a>> {
        self.tree
            .nodes
            .iter()
            .enumerate()
            .filter_map(move |(index, node)| match node.kind {
                SyntaxNodeKind::ProgramBlock(kind) => Some(ProgramBlock {
                    tree: self.tree,
                    index,
                    kind,
                }),
                _ => None,
            })
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ProgramBlock<'a> {
    tree: &'a SyntaxTree,
    index: usize,
    kind: ProgramBlockKind,
}

impl ProgramBlock<'_> {
    pub const fn kind(self) -> ProgramBlockKind {
        self.kind
    }

    pub fn range(self) -> TextRange {
        self.tree.nodes[self.index].range
    }
}

#[derive(Debug, Clone)]
pub struct ParseResult {
    pub tree: SyntaxTree,
    pub diagnostics: Vec<Diagnostic>,
}

pub fn parse(text: Arc<str>, tokens: Arc<[Token]>) -> ParseResult {
    Parser::new(text, tokens).run()
}

struct Parser {
    text: Arc<str>,
    tokens: Arc<[Token]>,
    significant: Vec<usize>,
    nodes: Vec<SyntaxNode>,
    diagnostics: Vec<Diagnostic>,
    delimiter_pairs: BTreeMap<usize, usize>,
}

impl Parser {
    fn new(text: Arc<str>, tokens: Arc<[Token]>) -> Self {
        let significant = tokens
            .iter()
            .enumerate()
            .filter_map(|(index, token)| (!is_trivia(token.kind)).then_some(index))
            .collect();
        Self {
            text,
            tokens,
            significant,
            nodes: Vec::new(),
            diagnostics: Vec::new(),
            delimiter_pairs: BTreeMap::new(),
        }
    }

    fn run(mut self) -> ParseResult {
        self.nodes.push(SyntaxNode {
            kind: SyntaxNodeKind::SourceFile,
            range: TextRange::new(0, self.text.len()),
            token_range: 0..self.tokens.len(),
            parent: None,
        });
        self.match_delimiters();
        self.structural_recovery();
        self.find_program_blocks();
        self.find_structures();
        self.find_includes_calls_and_statements();
        ParseResult {
            tree: SyntaxTree {
                text: self.text,
                tokens: self.tokens,
                nodes: self.nodes.into(),
            },
            diagnostics: self.diagnostics,
        }
    }

    fn match_delimiters(&mut self) {
        let mut stack = Vec::<(usize, Symbol)>::new();
        for index in self.significant.iter().copied() {
            let SyntaxKind::Symbol(symbol) = self.tokens[index].kind else {
                continue;
            };
            match symbol {
                Symbol::LeftBrace | Symbol::LeftParen | Symbol::LeftBracket => {
                    stack.push((index, symbol));
                }
                Symbol::RightBrace | Symbol::RightParen | Symbol::RightBracket => {
                    let expected = matching_open(symbol);
                    match stack.last().copied() {
                        Some((open_index, open)) if open == expected => {
                            stack.pop();
                            self.delimiter_pairs.insert(open_index, index);
                        }
                        _ => self.diagnostics.push(Diagnostic::error(
                            "syntax.unmatched-closing-delimiter",
                            format!("unmatched closing delimiter `{}`", symbol.as_str()),
                            self.tokens[index].range,
                        )),
                    }
                }
                _ => {}
            }
        }
        for (index, symbol) in stack {
            self.diagnostics.push(Diagnostic::error(
                "syntax.unclosed-delimiter",
                format!("unclosed delimiter `{}`", symbol.as_str()),
                self.tokens[index].range,
            ));
        }
    }

    fn find_program_blocks(&mut self) {
        let mut first_by_kind = BTreeMap::<ProgramBlockKind, TextRange>::new();
        let mut last_block_rank = None;
        let mut cursor = 0;
        while cursor < self.significant.len() {
            let Some((kind, header_len)) = self.block_header(cursor) else {
                cursor += 1;
                continue;
            };
            let brace_position = cursor + header_len;
            let Some(brace_index) = self.significant.get(brace_position).copied() else {
                break;
            };
            if self.tokens[brace_index].kind != SyntaxKind::Symbol(Symbol::LeftBrace) {
                cursor += 1;
                continue;
            }
            let end_index = self
                .delimiter_pairs
                .get(&brace_index)
                .copied()
                .unwrap_or(brace_index);
            let start_index = self.significant[cursor];
            let range = TextRange {
                start: self.tokens[start_index].range.start,
                end: self.tokens[end_index].range.end,
            };
            let rank = block_rank(kind);
            if last_block_rank.is_some_and(|previous| rank < previous) {
                self.diagnostics.push(Diagnostic::error(
                    "syntax.program-block-order",
                    format!("`{}` block appears out of order", kind.as_str()),
                    range,
                ));
            }
            last_block_rank = Some(rank);
            let node_index = self.nodes.len();
            self.nodes.push(SyntaxNode {
                kind: SyntaxNodeKind::ProgramBlock(kind),
                range,
                token_range: start_index..end_index.saturating_add(1),
                parent: Some(0),
            });
            if let Some(first) = first_by_kind.insert(kind, range) {
                let mut diagnostic = Diagnostic::error(
                    "syntax.duplicate-program-block",
                    format!("duplicate `{}` block", kind.as_str()),
                    range,
                );
                diagnostic.related.push(RelatedDiagnostic {
                    range: first,
                    message: "first block is here".to_owned(),
                });
                self.diagnostics.push(diagnostic);
            }
            let _ = node_index;
            cursor += header_len + 1;
        }
    }

    fn structural_recovery(&mut self) {
        for (position, index) in self.significant.iter().copied().enumerate() {
            if self.tokens[index].kind == SyntaxKind::Symbol(Symbol::RightBrace) {
                let Some(previous) = position
                    .checked_sub(1)
                    .and_then(|position| self.significant.get(position))
                    .copied()
                else {
                    continue;
                };
                if matches!(
                    self.tokens[previous].kind,
                    SyntaxKind::Identifier
                        | SyntaxKind::IntegerLiteral
                        | SyntaxKind::RealLiteral
                        | SyntaxKind::ImaginaryLiteral
                        | SyntaxKind::StringLiteral
                        | SyntaxKind::Symbol(Symbol::RightParen | Symbol::RightBracket)
                ) {
                    let mut diagnostic = Diagnostic::error(
                        "syntax.missing-semicolon",
                        "expected `;` before `}`",
                        TextRange {
                            start: self.tokens[index].range.start,
                            end: self.tokens[index].range.start,
                        },
                    );
                    diagnostic.fixes.push(Fix {
                        label: "insert `;`".to_owned(),
                        edits: vec![TextEdit {
                            range: TextRange {
                                start: self.tokens[index].range.start,
                                end: self.tokens[index].range.start,
                            },
                            replacement: ";".to_owned(),
                        }],
                        applicability: Applicability::Always,
                    });
                    self.diagnostics.push(diagnostic);
                }
            }
            if matches!(
                self.tokens[index].kind,
                SyntaxKind::Keyword(Keyword::Transformed | Keyword::Generated)
            ) && self.block_header(position).is_none()
            {
                self.diagnostics.push(Diagnostic::error(
                    "syntax.invalid-program-block-header",
                    "invalid compound program block header",
                    self.tokens[index].range,
                ));
            }
        }
    }

    fn block_header(&self, cursor: usize) -> Option<(ProgramBlockKind, usize)> {
        let keyword = |offset: usize| match self.significant.get(cursor + offset) {
            Some(index) => match self.tokens[*index].kind {
                SyntaxKind::Keyword(keyword) => Some(keyword),
                _ => None,
            },
            None => None,
        };
        match (keyword(0)?, keyword(1)) {
            (Keyword::Functions, _) => Some((ProgramBlockKind::Functions, 1)),
            (Keyword::Data, _) => Some((ProgramBlockKind::Data, 1)),
            (Keyword::Transformed, Some(Keyword::Data)) => {
                Some((ProgramBlockKind::TransformedData, 2))
            }
            (Keyword::Parameters, _) => Some((ProgramBlockKind::Parameters, 1)),
            (Keyword::Transformed, Some(Keyword::Parameters)) => {
                Some((ProgramBlockKind::TransformedParameters, 2))
            }
            (Keyword::Model, _) => Some((ProgramBlockKind::Model, 1)),
            (Keyword::Generated, Some(Keyword::Quantities)) => {
                Some((ProgramBlockKind::GeneratedQuantities, 2))
            }
            _ => None,
        }
    }

    fn find_structures(&mut self) {
        let significant = self.significant.clone();
        for (position, index) in significant.iter().copied().enumerate() {
            if self.tokens[index].kind == SyntaxKind::Keyword(Keyword::For) {
                let Some(open) = significant.get(position + 1).copied() else {
                    continue;
                };
                if self.tokens[open].kind != SyntaxKind::Symbol(Symbol::LeftParen) {
                    continue;
                }
                let Some(close) = self.delimiter_pairs.get(&open).copied() else {
                    continue;
                };
                let Some(brace) = significant
                    .iter()
                    .copied()
                    .find(|candidate| *candidate > close)
                else {
                    continue;
                };
                if self.tokens[brace].kind != SyntaxKind::Symbol(Symbol::LeftBrace) {
                    continue;
                }
                let end = self.delimiter_pairs.get(&brace).copied().unwrap_or(brace);
                self.nodes.push(SyntaxNode {
                    kind: SyntaxNodeKind::ForStatement,
                    range: TextRange {
                        start: self.tokens[index].range.start,
                        end: self.tokens[end].range.end,
                    },
                    token_range: index..end.saturating_add(1),
                    parent: Some(0),
                });
            }
            if is_type_keyword(self.tokens[index].kind) {
                self.nodes.push(SyntaxNode {
                    kind: SyntaxNodeKind::Type,
                    range: self.tokens[index].range,
                    token_range: index..index + 1,
                    parent: Some(0),
                });
            }
            if self.tokens[index].kind == SyntaxKind::Symbol(Symbol::LeftBrace) {
                let end = self.delimiter_pairs.get(&index).copied().unwrap_or(index);
                self.nodes.push(SyntaxNode {
                    kind: SyntaxNodeKind::CompoundStatement,
                    range: TextRange {
                        start: self.tokens[index].range.start,
                        end: self.tokens[end].range.end,
                    },
                    token_range: index..end.saturating_add(1),
                    parent: Some(0),
                });
            }
            if self.tokens[index].kind == SyntaxKind::Symbol(Symbol::LessThan)
                && position > 0
                && is_type_keyword(self.tokens[significant[position - 1]].kind)
            {
                if let Some(end) = significant[position + 1..]
                    .iter()
                    .copied()
                    .find(|candidate| {
                        self.tokens[*candidate].kind == SyntaxKind::Symbol(Symbol::GreaterThan)
                    })
                {
                    self.nodes.push(SyntaxNode {
                        kind: SyntaxNodeKind::Constraint,
                        range: TextRange {
                            start: self.tokens[index].range.start,
                            end: self.tokens[end].range.end,
                        },
                        token_range: index..end + 1,
                        parent: Some(0),
                    });
                }
            }
            if self.tokens[index].kind == SyntaxKind::Symbol(Symbol::LeftParen)
                && position > 0
                && self.tokens[significant[position - 1]].kind == SyntaxKind::Identifier
            {
                let declaration =
                    position >= 2 && is_type_keyword(self.tokens[significant[position - 2]].kind);
                if declaration {
                    let end = self.delimiter_pairs.get(&index).copied().unwrap_or(index);
                    self.nodes.push(SyntaxNode {
                        kind: SyntaxNodeKind::ParameterList,
                        range: TextRange {
                            start: self.tokens[index].range.start,
                            end: self.tokens[end].range.end,
                        },
                        token_range: index..end.saturating_add(1),
                        parent: Some(0),
                    });
                }
            }
        }
    }

    fn find_includes_calls_and_statements(&mut self) {
        let significant = self.significant.clone();
        let mut statement_start = 0usize;
        for (position, index) in significant.iter().copied().enumerate() {
            if position + 3 >= significant.len() || !is_type_keyword(self.tokens[index].kind) {
                continue;
            }
            let name = significant[position + 1];
            let open = significant[position + 2];
            if self.tokens[name].kind != SyntaxKind::Identifier
                || self.tokens[open].kind != SyntaxKind::Symbol(Symbol::LeftParen)
            {
                continue;
            }
            let Some(close) = self.delimiter_pairs.get(&open).copied() else {
                continue;
            };
            let Some(brace) = significant
                .iter()
                .copied()
                .find(|candidate| *candidate > close)
            else {
                continue;
            };
            if self.tokens[brace].kind != SyntaxKind::Symbol(Symbol::LeftBrace) {
                continue;
            }
            let end = self.delimiter_pairs.get(&brace).copied().unwrap_or(brace);
            self.nodes.push(SyntaxNode {
                kind: SyntaxNodeKind::FunctionDeclaration,
                range: TextRange {
                    start: self.tokens[index].range.start,
                    end: self.tokens[end].range.end,
                },
                token_range: index..end.saturating_add(1),
                parent: Some(0),
            });
        }
        for (position, index) in significant.iter().copied().enumerate() {
            match self.tokens[index].kind {
                SyntaxKind::Symbol(Symbol::LeftBrace | Symbol::RightBrace) => {
                    statement_start = position + 1;
                }
                SyntaxKind::Directive(_) => self.nodes.push(SyntaxNode {
                    kind: SyntaxNodeKind::Include,
                    range: self.tokens[index].range,
                    token_range: index..index + 1,
                    parent: Some(0),
                }),
                SyntaxKind::Identifier
                    if significant.get(position + 1).is_some_and(|next| {
                        self.tokens[*next].kind == SyntaxKind::Symbol(Symbol::LeftParen)
                    }) =>
                {
                    let open = significant[position + 1];
                    let end = self.delimiter_pairs.get(&open).copied().unwrap_or(open);
                    self.nodes.push(SyntaxNode {
                        kind: SyntaxNodeKind::FunctionCall,
                        range: TextRange {
                            start: self.tokens[index].range.start,
                            end: self.tokens[end].range.end,
                        },
                        token_range: index..end.saturating_add(1),
                        parent: Some(0),
                    });
                }
                SyntaxKind::Symbol(Symbol::Semicolon) => {
                    let start = significant.get(statement_start).copied().unwrap_or(index);
                    let contains_sampling = significant[statement_start..=position]
                        .iter()
                        .any(|item| self.tokens[*item].kind == SyntaxKind::Symbol(Symbol::Tilde));
                    let contains_declaration = significant[statement_start..position]
                        .iter()
                        .any(|item| is_type_keyword(self.tokens[*item].kind));
                    if contains_declaration
                        && !significant[statement_start..position]
                            .iter()
                            .any(|item| self.tokens[*item].kind == SyntaxKind::Identifier)
                    {
                        self.diagnostics.push(Diagnostic::error(
                            "syntax.malformed-declaration",
                            "declaration has no name",
                            TextRange {
                                start: self.tokens[start].range.start,
                                end: self.tokens[index].range.end,
                            },
                        ));
                    }
                    self.nodes.push(SyntaxNode {
                        kind: if contains_sampling {
                            SyntaxNodeKind::SamplingStatement
                        } else if contains_declaration {
                            SyntaxNodeKind::VariableDeclaration
                        } else {
                            SyntaxNodeKind::Statement
                        },
                        range: TextRange {
                            start: self.tokens[start].range.start,
                            end: self.tokens[index].range.end,
                        },
                        token_range: start..index + 1,
                        parent: Some(0),
                    });
                    if let Some(expression_start) = significant[statement_start..position]
                        .iter()
                        .position(|item| {
                            matches!(
                                self.tokens[*item].kind,
                                SyntaxKind::Symbol(Symbol::Assign | Symbol::Tilde)
                            )
                        })
                        .map(|relative| statement_start + relative + 1)
                    {
                        if let Some(expression) = significant.get(expression_start).copied() {
                            if let Err(range) = parse_expression_pratt(
                                &self.tokens,
                                &significant[expression_start..position],
                            ) {
                                self.diagnostics.push(Diagnostic::error(
                                    "syntax.expected-expression",
                                    "expected a valid expression",
                                    range,
                                ));
                            }
                            self.nodes.push(SyntaxNode {
                                kind: SyntaxNodeKind::Expression,
                                range: TextRange {
                                    start: self.tokens[expression].range.start,
                                    end: self.tokens[index].range.start,
                                },
                                token_range: expression..index,
                                parent: Some(0),
                            });
                        }
                    }
                    statement_start = position + 1;
                }
                _ => {}
            }
        }
    }
}

fn is_trivia(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        SyntaxKind::Whitespace | SyntaxKind::LineComment | SyntaxKind::BlockComment
    )
}

fn is_type_keyword(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        SyntaxKind::Keyword(
            Keyword::Void
                | Keyword::Int
                | Keyword::Real
                | Keyword::Complex
                | Keyword::Vector
                | Keyword::RowVector
                | Keyword::Matrix
                | Keyword::ComplexVector
                | Keyword::ComplexRowVector
                | Keyword::ComplexMatrix
                | Keyword::Array
                | Keyword::Tuple
        )
    )
}

fn matching_open(close: Symbol) -> Symbol {
    match close {
        Symbol::RightBrace => Symbol::LeftBrace,
        Symbol::RightParen => Symbol::LeftParen,
        Symbol::RightBracket => Symbol::LeftBracket,
        _ => unreachable!("called only for closing delimiters"),
    }
}

const fn block_rank(kind: ProgramBlockKind) -> u8 {
    match kind {
        ProgramBlockKind::Functions => 0,
        ProgramBlockKind::Data => 1,
        ProgramBlockKind::TransformedData => 2,
        ProgramBlockKind::Parameters => 3,
        ProgramBlockKind::TransformedParameters => 4,
        ProgramBlockKind::Model => 5,
        ProgramBlockKind::GeneratedQuantities => 6,
    }
}

fn parse_expression_pratt(tokens: &[Token], indices: &[usize]) -> Result<(), TextRange> {
    if indices.is_empty() {
        return Err(TextRange::new(0, 0));
    }
    let mut parser = ExpressionParser {
        tokens,
        indices,
        position: 0,
    };
    parser.parse_binding_power(0)?;
    if parser.position == indices.len() {
        Ok(())
    } else {
        Err(tokens[indices[parser.position]].range)
    }
}

struct ExpressionParser<'a> {
    tokens: &'a [Token],
    indices: &'a [usize],
    position: usize,
}

impl ExpressionParser<'_> {
    fn parse_binding_power(&mut self, minimum: u8) -> Result<(), TextRange> {
        let token = self.bump().ok_or_else(|| self.end_range())?;
        match token.kind {
            SyntaxKind::Identifier
            | SyntaxKind::IntegerLiteral
            | SyntaxKind::RealLiteral
            | SyntaxKind::ImaginaryLiteral
            | SyntaxKind::StringLiteral
            | SyntaxKind::Keyword(Keyword::Target) => {}
            SyntaxKind::Symbol(Symbol::LeftParen) => {
                self.parse_binding_power(0)?;
                self.expect(Symbol::RightParen)?;
            }
            SyntaxKind::Symbol(symbol)
                if symbol
                    .operator_forms()
                    .iter()
                    .any(|form| form.fixity == crate::Fixity::Prefix) =>
            {
                self.parse_binding_power(10)?;
            }
            _ => return Err(token.range),
        }

        loop {
            let Some(next) = self.peek() else {
                break;
            };
            match next.kind {
                SyntaxKind::Symbol(Symbol::LeftParen) => {
                    self.bump();
                    if self.peek_kind() != Some(SyntaxKind::Symbol(Symbol::RightParen)) {
                        loop {
                            self.parse_binding_power(0)?;
                            if self.peek_kind() == Some(SyntaxKind::Symbol(Symbol::Comma)) {
                                self.bump();
                            } else {
                                break;
                            }
                        }
                    }
                    self.expect(Symbol::RightParen)?;
                }
                SyntaxKind::Symbol(Symbol::LeftBracket) => {
                    self.bump();
                    if self.peek_kind() != Some(SyntaxKind::Symbol(Symbol::RightBracket)) {
                        loop {
                            self.parse_binding_power(0)?;
                            if self.peek_kind() == Some(SyntaxKind::Symbol(Symbol::Comma)) {
                                self.bump();
                            } else {
                                break;
                            }
                        }
                    }
                    self.expect(Symbol::RightBracket)?;
                }
                SyntaxKind::Symbol(Symbol::Transpose) => {
                    self.bump();
                }
                SyntaxKind::Symbol(symbol) => {
                    let Some(form) = symbol
                        .operator_forms()
                        .iter()
                        .find(|form| form.fixity == crate::Fixity::Infix)
                    else {
                        break;
                    };
                    if form.precedence < minimum {
                        break;
                    }
                    self.bump();
                    let next_minimum = match form.associativity {
                        crate::Associativity::Right => form.precedence,
                        crate::Associativity::Left | crate::Associativity::NonAssociative => {
                            form.precedence + 1
                        }
                    };
                    self.parse_binding_power(next_minimum)?;
                }
                _ => break,
            }
        }
        Ok(())
    }

    fn expect(&mut self, symbol: Symbol) -> Result<(), TextRange> {
        match self.bump() {
            Some(token) if token.kind == SyntaxKind::Symbol(symbol) => Ok(()),
            Some(token) => Err(token.range),
            None => Err(self.end_range()),
        }
    }

    fn peek(&self) -> Option<Token> {
        self.indices
            .get(self.position)
            .map(|index| self.tokens[*index])
    }

    fn peek_kind(&self) -> Option<SyntaxKind> {
        self.peek().map(|token| token.kind)
    }

    fn bump(&mut self) -> Option<Token> {
        let token = self.peek()?;
        self.position += 1;
        Some(token)
    }

    fn end_range(&self) -> TextRange {
        self.indices.last().map_or(TextRange::new(0, 0), |index| {
            let end = self.tokens[*index].range.end as usize;
            TextRange::new(end, end)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lex;

    fn parse_source(source: &str) -> ParseResult {
        let lexed = lex(source);
        parse(source.into(), lexed.tokens.into())
    }

    #[test]
    fn tree_is_lossless_and_finds_blocks_calls_and_sampling() {
        let source = "data { real y; } model { y ~ normal(0, 1); }";
        let result = parse_source(source);
        assert_eq!(result.tree.reconstruct(), source);
        assert_eq!(result.tree.source_file().program_blocks().count(), 2);
        assert!(
            result
                .tree
                .nodes()
                .iter()
                .any(|node| node.kind == SyntaxNodeKind::FunctionCall)
        );
        assert!(
            result
                .tree
                .nodes()
                .iter()
                .any(|node| node.kind == SyntaxNodeKind::SamplingStatement)
        );
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn incomplete_source_returns_a_tree_and_diagnostics() {
        let result = parse_source("model { normal(");
        assert_eq!(result.tree.reconstruct(), "model { normal(");
        assert_eq!(
            result
                .diagnostics
                .iter()
                .filter(|diagnostic| diagnostic.code.0 == "syntax.unclosed-delimiter")
                .count(),
            2
        );
    }

    #[test]
    fn duplicate_blocks_have_related_information() {
        let result = parse_source("data {} data {}");
        let diagnostic = result
            .diagnostics
            .iter()
            .find(|diagnostic| diagnostic.code.0 == "syntax.duplicate-program-block")
            .unwrap();
        assert_eq!(diagnostic.related.len(), 1);
    }

    #[test]
    fn missing_semicolon_has_an_always_applicable_fix() {
        let result = parse_source("model { print(1) }");
        let diagnostic = result
            .diagnostics
            .iter()
            .find(|diagnostic| diagnostic.code.0 == "syntax.missing-semicolon")
            .unwrap();
        assert_eq!(diagnostic.fixes[0].applicability, Applicability::Always);
    }

    #[test]
    fn program_block_order_is_checked() {
        let result = parse_source("model {} data {}");
        assert!(
            result
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code.0 == "syntax.program-block-order")
        );
    }

    #[test]
    fn arbitrary_editor_input_always_returns_a_lossless_tree() {
        let alphabet = [
            '{', '}', '(', ')', '[', ']', ';', '"', '/', '*', 'α', '\n', '1', '_',
        ];
        let mut state = 0x1234_5678_u64;
        for _ in 0..1_000 {
            let mut source = String::new();
            for _ in 0..32 {
                state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
                source.push(alphabet[(state as usize) % alphabet.len()]);
            }
            let result = parse_source(&source);
            assert_eq!(result.tree.reconstruct(), source);
        }
    }

    #[test]
    fn typed_wrappers_cover_declarations_constraints_and_expressions() {
        let result = parse_source("parameters { real<lower=0> sigma; } model { sigma = exp(1); }");
        for kind in [
            SyntaxNodeKind::Type,
            SyntaxNodeKind::Constraint,
            SyntaxNodeKind::VariableDeclaration,
            SyntaxNodeKind::Expression,
            SyntaxNodeKind::FunctionCall,
        ] {
            assert!(
                result.tree.nodes().iter().any(|node| node.kind == kind),
                "{kind:?}"
            );
        }
    }

    #[test]
    fn pratt_parser_respects_precedence_and_reports_incomplete_rhs() {
        let valid = parse_source("model { real x; x = 1 + 2 * pow(3, 4); }");
        assert!(
            !valid
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code.0 == "syntax.expected-expression")
        );
        let invalid = parse_source("model { real x; x = 1 + ; }");
        assert!(
            invalid
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code.0 == "syntax.expected-expression")
        );
    }
}
