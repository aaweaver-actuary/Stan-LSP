//! A lossless, recovery-oriented concrete syntax model.

use std::{collections::BTreeMap, sync::Arc};

use crate::{
    Applicability, Diagnostic, Fix, Keyword, ProgramBlockKind, RelatedDiagnostic, Symbol,
    SyntaxKind, TextEdit, TextRange, Token,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
/// Structural classification of a recovery-tree node.
#[allow(
    missing_docs,
    reason = "variants are the documented syntax-node inventory"
)]
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
/// Immutable untyped node in the flat lossless recovery tree.
#[allow(
    missing_docs,
    reason = "fields are structural storage consumed through typed wrappers"
)]
pub struct SyntaxNode {
    pub kind: SyntaxNodeKind,
    pub range: TextRange,
    pub token_range: std::ops::Range<usize>,
    pub parent: Option<usize>,
}

#[derive(Debug, Clone)]
/// Lossless source text, tokens, and recovery nodes for arbitrary editor input.
pub struct SyntaxTree {
    text: Arc<str>,
    tokens: Arc<[Token]>,
    nodes: Arc<[SyntaxNode]>,
}

impl SyntaxTree {
    /// Returns the original source text.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Returns the complete lossless token stream.
    pub fn tokens(&self) -> &[Token] {
        &self.tokens
    }

    /// Returns untyped recovery nodes; prefer typed [`SourceFile`] queries.
    pub fn nodes(&self) -> &[SyntaxNode] {
        &self.nodes
    }

    /// Returns the typed root view.
    pub fn source_file(&self) -> SourceFile<'_> {
        SourceFile { tree: self }
    }

    /// Reconstructs the original source exactly from token ranges.
    pub fn reconstruct(&self) -> String {
        self.tokens
            .iter()
            .filter(|token| token.kind != SyntaxKind::EndOfFile)
            .map(|token| &self.text[token.range.start as usize..token.range.end as usize])
            .collect()
    }
}

#[derive(Debug, Clone, Copy)]
/// Typed root view used to enumerate supported syntax constructs.
///
/// Iterators return constructs in UTF-8 source order. Recovery may omit a
/// construct that cannot be identified without guessing its grammar role.
pub struct SourceFile<'a> {
    tree: &'a SyntaxTree,
}

impl<'a> SourceFile<'a> {
    /// Enumerates program blocks in source order.
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

    /// Enumerates recognized user-function declarations in source order.
    ///
    /// A declaration with an unterminated parameter list is retained when its
    /// return type and name are recognizable. Its optional accessors then
    /// report only the structure that is actually present.
    pub fn function_declarations(self) -> impl Iterator<Item = FunctionDeclaration<'a>> {
        nodes_of_kind(self.tree, SyntaxNodeKind::FunctionDeclaration).map(|index| {
            FunctionDeclaration {
                tree: self.tree,
                index,
            }
        })
    }

    /// Enumerates complete and recoverable variable declarations in source order.
    pub fn variable_declarations(self) -> impl Iterator<Item = VariableDeclaration<'a>> {
        nodes_of_kind(self.tree, SyntaxNodeKind::VariableDeclaration).map(|index| {
            VariableDeclaration {
                tree: self.tree,
                index,
            }
        })
    }

    /// Enumerates `for` statements with recognized binders and optional bodies.
    pub fn for_statements(self) -> impl Iterator<Item = ForStatement<'a>> {
        nodes_of_kind(self.tree, SyntaxNodeKind::ForStatement).map(|index| ForStatement {
            tree: self.tree,
            index,
        })
    }

    /// Enumerates brace-delimited compound statements.
    pub fn compound_statements(self) -> impl Iterator<Item = CompoundStatement<'a>> {
        nodes_of_kind(self.tree, SyntaxNodeKind::CompoundStatement).map(|index| CompoundStatement {
            tree: self.tree,
            index,
        })
    }

    /// Enumerates syntactically recognized call expressions, including incomplete calls.
    pub fn call_expressions(self) -> impl Iterator<Item = CallExpression<'a>> {
        nodes_of_kind(self.tree, SyntaxNodeKind::FunctionCall).map(|index| CallExpression {
            tree: self.tree,
            index,
        })
    }

    /// Enumerates sampling statements, including a final incomplete statement.
    pub fn sampling_statements(self) -> impl Iterator<Item = SamplingStatement<'a>> {
        nodes_of_kind(self.tree, SyntaxNodeKind::SamplingStatement).map(|index| SamplingStatement {
            tree: self.tree,
            index,
        })
    }
}

#[derive(Debug, Clone, Copy)]
/// Typed program-block header and range.
///
/// The range uses UTF-8 byte offsets and ends at the opening brace when the
/// block body is unterminated.
pub struct ProgramBlock<'a> {
    tree: &'a SyntaxTree,
    index: usize,
    kind: ProgramBlockKind,
}

impl ProgramBlock<'_> {
    /// Returns the grammar-level block kind.
    pub const fn kind(self) -> ProgramBlockKind {
        self.kind
    }

    /// Returns the complete header-and-body range.
    pub fn range(self) -> TextRange {
        self.tree.nodes[self.index].range
    }
}

#[derive(Debug, Clone, Copy)]
/// Borrowed identifier spelling and exact source range.
pub struct NameRef<'a> {
    tree: &'a SyntaxTree,
    range: TextRange,
}

impl<'a> NameRef<'a> {
    /// Returns the identifier's UTF-8 byte range.
    pub const fn range(self) -> TextRange {
        self.range
    }

    /// Returns the identifier spelling.
    pub fn text(self) -> &'a str {
        text_for_range(self.tree, self.range)
    }
}

#[derive(Debug, Clone, Copy)]
/// Borrowed type spelling retained exactly as written.
///
/// Ranges use UTF-8 byte offsets and may include constraints or dimensions.
pub struct TypeSyntax<'a> {
    tree: &'a SyntaxTree,
    range: TextRange,
}

impl<'a> TypeSyntax<'a> {
    /// Returns the type spelling's source range.
    pub const fn range(self) -> TextRange {
        self.range
    }

    /// Returns the original type spelling, including dimensions and constraints.
    pub fn text(self) -> &'a str {
        text_for_range(self.tree, self.range)
    }
}

#[derive(Debug, Clone, Copy)]
/// One name introduced by a variable declaration.
///
/// Declarators are returned in source order. The range includes any dimensions
/// or initializer belonging to this declarator, but excludes the separating
/// comma and terminating semicolon.
pub struct Declarator<'a> {
    tree: &'a SyntaxTree,
    range: TextRange,
    name_range: TextRange,
}

impl<'a> Declarator<'a> {
    /// Returns the declarator portion of the declaration.
    pub const fn range(self) -> TextRange {
        self.range
    }

    /// Returns the declared name.
    pub fn name(self) -> NameRef<'a> {
        NameRef {
            tree: self.tree,
            range: self.name_range,
        }
    }
}

#[derive(Debug, Clone, Copy)]
/// Structured function parameter, including qualifier and type spelling.
///
/// Parameters that do not yet contain both a recognizable type and name are
/// omitted from [`FunctionDeclaration::parameters`]. Returned ranges use UTF-8
/// byte offsets and parameters remain in source order.
pub struct FunctionParameter<'a> {
    tree: &'a SyntaxTree,
    range: TextRange,
    name_range: TextRange,
    type_range: TextRange,
    data_only: bool,
}

impl<'a> FunctionParameter<'a> {
    /// Returns the complete parameter range.
    pub const fn range(self) -> TextRange {
        self.range
    }

    /// Returns the introduced parameter name.
    pub fn name(self) -> NameRef<'a> {
        NameRef {
            tree: self.tree,
            range: self.name_range,
        }
    }

    /// Returns the original parameter type spelling.
    pub fn type_syntax(self) -> TypeSyntax<'a> {
        TypeSyntax {
            tree: self.tree,
            range: self.type_range,
        }
    }

    /// Reports whether the parameter carries Stan's `data` qualifier.
    pub const fn is_data_only(self) -> bool {
        self.data_only
    }
}

#[derive(Debug, Clone, Copy)]
/// Recovery-safe typed view of a recognized user-defined function declaration.
///
/// The name, return type, complete parameters, and body are independently
/// optional while the user is editing. All ranges use UTF-8 byte offsets.
pub struct FunctionDeclaration<'a> {
    tree: &'a SyntaxTree,
    index: usize,
}

impl<'a> FunctionDeclaration<'a> {
    /// Returns the declaration range.
    pub fn range(self) -> TextRange {
        self.tree.nodes[self.index].range
    }

    /// Returns the function name when structurally present.
    pub fn name(self) -> Option<NameRef<'a>> {
        let indices = significant_node_tokens(self.tree, self.index);
        indices.windows(2).find_map(|window| {
            (self.tree.tokens[window[0]].kind == SyntaxKind::Identifier
                && self.tree.tokens[window[1]].kind == SyntaxKind::Symbol(Symbol::LeftParen))
            .then_some(NameRef {
                tree: self.tree,
                range: self.tree.tokens[window[0]].range,
            })
        })
    }

    /// Returns the original return-type spelling.
    pub fn return_type(self) -> Option<TypeSyntax<'a>> {
        let name = self.name()?;
        let node = &self.tree.nodes[self.index];
        Some(TypeSyntax {
            tree: self.tree,
            range: TextRange {
                start: node.range.start,
                end: name.range.start,
            },
        })
    }

    /// Returns structurally complete parameters in source order.
    pub fn parameters(self) -> Vec<FunctionParameter<'a>> {
        let indices = significant_node_tokens(self.tree, self.index);
        let Some(name_position) = indices.iter().position(|index| {
            self.tree.tokens[*index].kind == SyntaxKind::Identifier
                && indices
                    .get(
                        indices
                            .iter()
                            .position(|candidate| candidate == index)
                            .unwrap_or(0)
                            + 1,
                    )
                    .is_some_and(|next| {
                        self.tree.tokens[*next].kind == SyntaxKind::Symbol(Symbol::LeftParen)
                    })
        }) else {
            return Vec::new();
        };
        let open_position = name_position + 1;
        let close_position = matching_position(
            self.tree,
            &indices,
            open_position,
            Symbol::LeftParen,
            Symbol::RightParen,
        )
        .unwrap_or(indices.len());
        split_top_level_indices(
            self.tree,
            &indices[open_position + 1..close_position],
            Symbol::Comma,
        )
        .into_iter()
        .filter_map(|segment| parameter_from_indices(self.tree, &segment))
        .collect()
    }

    /// Returns the brace-delimited body range when complete.
    pub fn body_range(self) -> Option<TextRange> {
        let indices = significant_node_tokens(self.tree, self.index);
        let open_position = indices.iter().position(|index| {
            self.tree.tokens[*index].kind == SyntaxKind::Symbol(Symbol::LeftBrace)
        })?;
        let close_position = matching_position(
            self.tree,
            &indices,
            open_position,
            Symbol::LeftBrace,
            Symbol::RightBrace,
        )?;
        Some(TextRange {
            start: self.tree.tokens[indices[open_position]].range.start,
            end: self.tree.tokens[indices[close_position]].range.end,
        })
    }
}

#[derive(Debug, Clone, Copy)]
/// Recovery-safe typed variable declaration view.
///
/// A final declaration without a semicolon may still be represented. Missing
/// names produce no declarators, and all ranges use UTF-8 byte offsets.
pub struct VariableDeclaration<'a> {
    tree: &'a SyntaxTree,
    index: usize,
}

impl<'a> VariableDeclaration<'a> {
    /// Returns the recognized declaration range, including a semicolon if present.
    pub fn range(self) -> TextRange {
        self.tree.nodes[self.index].range
    }

    /// Returns the original type spelling.
    pub fn type_syntax(self) -> Option<TypeSyntax<'a>> {
        let indices = significant_node_tokens(self.tree, self.index);
        let name_position = declaration_name_position(self.tree, &indices)?;
        Some(TypeSyntax {
            tree: self.tree,
            range: TextRange {
                start: self.tree.tokens[*indices.first()?].range.start,
                end: self.tree.tokens[indices[name_position]].range.start,
            },
        })
    }

    /// Returns names introduced by this declaration in source order.
    pub fn declarators(self) -> Vec<Declarator<'a>> {
        let indices = significant_node_tokens(self.tree, self.index);
        let Some(name_position) = declaration_name_position(self.tree, &indices) else {
            return Vec::new();
        };
        split_top_level_indices(
            self.tree,
            &indices[name_position..]
                .iter()
                .copied()
                .filter(|index| {
                    self.tree.tokens[*index].kind != SyntaxKind::Symbol(Symbol::Semicolon)
                })
                .collect::<Vec<_>>(),
            Symbol::Comma,
        )
        .into_iter()
        .filter_map(|segment| {
            let name_index = segment
                .iter()
                .copied()
                .find(|index| self.tree.tokens[*index].kind == SyntaxKind::Identifier)?;
            Some(Declarator {
                tree: self.tree,
                range: TextRange {
                    start: self.tree.tokens[*segment.first()?].range.start,
                    end: self.tree.tokens[*segment.last()?].range.end,
                },
                name_range: self.tree.tokens[name_index].range,
            })
        })
        .collect()
    }
}

#[derive(Debug, Clone, Copy)]
/// Recovery-safe typed `for` statement with a scoped binder.
///
/// The binder and body may be absent on incomplete input. Ranges use UTF-8 byte
/// offsets.
pub struct ForStatement<'a> {
    tree: &'a SyntaxTree,
    index: usize,
}

impl<'a> ForStatement<'a> {
    /// Returns the recognized loop range.
    pub fn range(self) -> TextRange {
        self.tree.nodes[self.index].range
    }

    /// Returns the loop binder when structurally present.
    pub fn binder(self) -> Option<NameRef<'a>> {
        let indices = significant_node_tokens(self.tree, self.index);
        let in_position = indices
            .iter()
            .position(|index| self.tree.tokens[*index].kind == SyntaxKind::Keyword(Keyword::In))?;
        indices[..in_position].iter().rev().find_map(|index| {
            (self.tree.tokens[*index].kind == SyntaxKind::Identifier).then_some(NameRef {
                tree: self.tree,
                range: self.tree.tokens[*index].range,
            })
        })
    }

    /// Returns the brace-delimited body when complete.
    pub fn body_range(self) -> Option<TextRange> {
        let indices = significant_node_tokens(self.tree, self.index);
        let open_position = indices.iter().position(|index| {
            self.tree.tokens[*index].kind == SyntaxKind::Symbol(Symbol::LeftBrace)
        })?;
        let close_position = matching_position(
            self.tree,
            &indices,
            open_position,
            Symbol::LeftBrace,
            Symbol::RightBrace,
        )?;
        Some(TextRange {
            start: self.tree.tokens[indices[open_position]].range.start,
            end: self.tree.tokens[indices[close_position]].range.end,
        })
    }
}

#[derive(Debug, Clone, Copy)]
/// Typed brace-delimited compound statement.
///
/// An unmatched opening brace is represented by its one-byte UTF-8 range.
pub struct CompoundStatement<'a> {
    tree: &'a SyntaxTree,
    index: usize,
}

impl CompoundStatement<'_> {
    /// Returns the brace-delimited source range.
    pub fn range(self) -> TextRange {
        self.tree.nodes[self.index].range
    }
}

#[derive(Debug, Clone, Copy)]
/// Typed function call, which may be incomplete during editing.
///
/// Missing closing delimiters are reported through [`Self::is_complete`]
/// rather than synthesized. Ranges use UTF-8 byte offsets.
pub struct CallExpression<'a> {
    tree: &'a SyntaxTree,
    index: usize,
}

impl<'a> CallExpression<'a> {
    /// Returns the recognized call range.
    pub fn range(self) -> TextRange {
        self.tree.nodes[self.index].range
    }

    /// Returns the callee when present.
    pub fn callee(self) -> Option<NameRef<'a>> {
        let index = significant_node_tokens(self.tree, self.index)
            .into_iter()
            .find(|index| self.tree.tokens[*index].kind == SyntaxKind::Identifier)?;
        Some(NameRef {
            tree: self.tree,
            range: self.tree.tokens[index].range,
        })
    }

    /// Returns the closing parenthesis range when present.
    pub fn closing_paren(self) -> Option<TextRange> {
        significant_node_tokens(self.tree, self.index)
            .into_iter()
            .rev()
            .find_map(|index| {
                (self.tree.tokens[index].kind == SyntaxKind::Symbol(Symbol::RightParen))
                    .then_some(self.tree.tokens[index].range)
            })
    }

    /// Reports whether semantic call checks have enough structure to run.
    pub fn is_complete(self) -> bool {
        self.callee().is_some() && self.closing_paren().is_some()
    }

    /// Reports whether this parenthesized construct is a function declaration header.
    pub fn is_declaration(self) -> bool {
        let Some(callee) = self.callee() else {
            return false;
        };
        self.tree
            .source_file()
            .function_declarations()
            .any(|function| {
                function
                    .name()
                    .is_some_and(|name| name.range() == callee.range())
            })
    }
}

#[derive(Debug, Clone, Copy)]
/// Typed sampling statement, which may retain incomplete inner syntax.
///
/// The final statement may lack both its closing parenthesis and semicolon.
/// Ranges use UTF-8 byte offsets.
pub struct SamplingStatement<'a> {
    tree: &'a SyntaxTree,
    index: usize,
}

impl<'a> SamplingStatement<'a> {
    /// Returns the complete semicolon-terminated statement range.
    pub fn range(self) -> TextRange {
        self.tree.nodes[self.index].range
    }

    /// Returns the distribution spelling after `~`.
    pub fn distribution(self) -> Option<NameRef<'a>> {
        let indices = significant_node_tokens(self.tree, self.index);
        let tilde = indices
            .iter()
            .position(|index| self.tree.tokens[*index].kind == SyntaxKind::Symbol(Symbol::Tilde))?;
        indices[tilde + 1..].iter().find_map(|index| {
            (self.tree.tokens[*index].kind == SyntaxKind::Identifier).then_some(NameRef {
                tree: self.tree,
                range: self.tree.tokens[*index].range,
            })
        })
    }

    /// Returns the closing call parenthesis when present.
    pub fn closing_paren(self) -> Option<TextRange> {
        significant_node_tokens(self.tree, self.index)
            .into_iter()
            .rev()
            .find_map(|index| {
                (self.tree.tokens[index].kind == SyntaxKind::Symbol(Symbol::RightParen))
                    .then_some(self.tree.tokens[index].range)
            })
    }

    /// Reports whether distribution diagnostics have enough structure to run.
    pub fn is_complete(self) -> bool {
        self.distribution().is_some() && self.closing_paren().is_some()
    }
}

fn nodes_of_kind(tree: &SyntaxTree, kind: SyntaxNodeKind) -> impl Iterator<Item = usize> + '_ {
    tree.nodes
        .iter()
        .enumerate()
        .filter_map(move |(index, node)| (node.kind == kind).then_some(index))
}

fn significant_node_tokens(tree: &SyntaxTree, index: usize) -> Vec<usize> {
    tree.nodes[index]
        .token_range
        .clone()
        .filter(|token| !is_trivia(tree.tokens[*token].kind))
        .collect()
}

fn text_for_range(tree: &SyntaxTree, range: TextRange) -> &str {
    &tree.text[range.start as usize..range.end as usize]
}

fn matching_position(
    tree: &SyntaxTree,
    indices: &[usize],
    open_position: usize,
    open: Symbol,
    close: Symbol,
) -> Option<usize> {
    let mut depth = 0usize;
    for (position, index) in indices.iter().copied().enumerate().skip(open_position) {
        match tree.tokens[index].kind {
            SyntaxKind::Symbol(symbol) if symbol == open => depth += 1,
            SyntaxKind::Symbol(symbol) if symbol == close => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(position);
                }
            }
            _ => {}
        }
    }
    None
}

fn split_top_level_indices(
    tree: &SyntaxTree,
    indices: &[usize],
    separator: Symbol,
) -> Vec<Vec<usize>> {
    let mut output = Vec::new();
    let mut start = 0usize;
    let mut parens = 0usize;
    let mut brackets = 0usize;
    for (position, index) in indices.iter().copied().enumerate() {
        match tree.tokens[index].kind {
            SyntaxKind::Symbol(Symbol::LeftParen) => parens += 1,
            SyntaxKind::Symbol(Symbol::RightParen) => parens = parens.saturating_sub(1),
            SyntaxKind::Symbol(Symbol::LeftBracket) => brackets += 1,
            SyntaxKind::Symbol(Symbol::RightBracket) => brackets = brackets.saturating_sub(1),
            SyntaxKind::Symbol(symbol) if symbol == separator && parens == 0 && brackets == 0 => {
                output.push(indices[start..position].to_vec());
                start = position + 1;
            }
            _ => {}
        }
    }
    if start < indices.len() {
        output.push(indices[start..].to_vec());
    }
    output
}

fn parameter_from_indices<'a>(
    tree: &'a SyntaxTree,
    indices: &[usize],
) -> Option<FunctionParameter<'a>> {
    let name_position = declaration_name_position(tree, indices)?;
    let name_index = indices[name_position];
    let first = *indices.first()?;
    let type_start = indices
        .iter()
        .copied()
        .find(|index| tree.tokens[*index].kind != SyntaxKind::Keyword(Keyword::Data))?;
    Some(FunctionParameter {
        tree,
        range: TextRange {
            start: tree.tokens[first].range.start,
            end: tree.tokens[*indices.last()?].range.end,
        },
        name_range: tree.tokens[name_index].range,
        type_range: TextRange {
            start: tree.tokens[type_start].range.start,
            end: tree.tokens[name_index].range.start,
        },
        data_only: tree.tokens[first].kind == SyntaxKind::Keyword(Keyword::Data),
    })
}

fn declaration_name_position(tree: &SyntaxTree, indices: &[usize]) -> Option<usize> {
    let mut saw_type = false;
    let mut parens = 0usize;
    let mut brackets = 0usize;
    let mut angles = 0usize;
    for (position, index) in indices.iter().copied().enumerate() {
        match tree.tokens[index].kind {
            kind if is_type_keyword(kind) => saw_type = true,
            SyntaxKind::Symbol(Symbol::LeftParen) if saw_type => parens += 1,
            SyntaxKind::Symbol(Symbol::RightParen) if saw_type => parens = parens.saturating_sub(1),
            SyntaxKind::Symbol(Symbol::LeftBracket) if saw_type => brackets += 1,
            SyntaxKind::Symbol(Symbol::RightBracket) if saw_type => {
                brackets = brackets.saturating_sub(1)
            }
            SyntaxKind::Symbol(Symbol::LessThan) if saw_type => angles += 1,
            SyntaxKind::Symbol(Symbol::GreaterThan) if saw_type => {
                angles = angles.saturating_sub(1)
            }
            SyntaxKind::Identifier if saw_type && parens == 0 && brackets == 0 && angles == 0 => {
                return Some(position);
            }
            _ => {}
        }
    }
    None
}

#[derive(Debug, Clone)]
/// Parser output containing a lossless tree and recoverable structural diagnostics.
pub struct ParseResult {
    /// Always-present lossless recovery tree.
    pub tree: SyntaxTree,
    /// High-confidence structural findings.
    pub diagnostics: Vec<Diagnostic>,
}

/// Parses a lossless token stream without rejecting incomplete editor input.
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
                    let end =
                        last_source_token(&self.tokens, &significant[position..]).unwrap_or(open);
                    self.nodes.push(SyntaxNode {
                        kind: SyntaxNodeKind::ForStatement,
                        range: TextRange {
                            start: self.tokens[index].range.start,
                            end: self.tokens[end].range.end,
                        },
                        token_range: index..end.saturating_add(1),
                        parent: Some(0),
                    });
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
            if self.tokens[index].kind != SyntaxKind::Identifier {
                continue;
            }
            let Some(open) = significant.get(position + 1).copied() else {
                continue;
            };
            if self.tokens[open].kind != SyntaxKind::Symbol(Symbol::LeftParen) {
                continue;
            }
            let start_position = significant[..position]
                .iter()
                .rposition(|candidate| {
                    matches!(
                        self.tokens[*candidate].kind,
                        SyntaxKind::Symbol(
                            Symbol::LeftBrace | Symbol::RightBrace | Symbol::Semicolon
                        )
                    )
                })
                .map_or(0, |boundary| boundary + 1);
            if !significant[start_position..position]
                .iter()
                .any(|candidate| is_type_keyword(self.tokens[*candidate].kind))
            {
                continue;
            }
            let start = significant[start_position];
            let Some(close) = self.delimiter_pairs.get(&open).copied() else {
                let end = last_source_token(&self.tokens, &significant[position..]).unwrap_or(open);
                self.nodes.push(SyntaxNode {
                    kind: SyntaxNodeKind::FunctionDeclaration,
                    range: TextRange {
                        start: self.tokens[start].range.start,
                        end: self.tokens[end].range.end,
                    },
                    token_range: start..end.saturating_add(1),
                    parent: Some(0),
                });
                continue;
            };
            let Some(close_position) = significant.iter().position(|candidate| *candidate == close)
            else {
                continue;
            };
            let Some(brace) = significant.get(close_position + 1).copied() else {
                continue;
            };
            if self.tokens[brace].kind != SyntaxKind::Symbol(Symbol::LeftBrace) {
                continue;
            }
            let end = self.delimiter_pairs.get(&brace).copied().unwrap_or(brace);
            self.nodes.push(SyntaxNode {
                kind: SyntaxNodeKind::FunctionDeclaration,
                range: TextRange {
                    start: self.tokens[start].range.start,
                    end: self.tokens[end].range.end,
                },
                token_range: start..end.saturating_add(1),
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
                    if let Some(expression_start) = top_level_expression_start(
                        &self.tokens,
                        &significant[statement_start..position],
                    )
                    .map(|relative| statement_start + relative)
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
        let tail = &significant[statement_start..];
        if let (Some(start), Some(end)) =
            (tail.first().copied(), last_source_token(&self.tokens, tail))
        {
            let tail = tail
                .iter()
                .copied()
                .take_while(|index| *index <= end)
                .collect::<Vec<_>>();
            let contains_sampling = tail
                .iter()
                .any(|index| self.tokens[*index].kind == SyntaxKind::Symbol(Symbol::Tilde));
            let contains_declaration = tail
                .iter()
                .any(|index| is_type_keyword(self.tokens[*index].kind));
            let overlaps_function = self.nodes.iter().any(|node| {
                node.kind == SyntaxNodeKind::FunctionDeclaration
                    && node.range.start == self.tokens[start].range.start
            });
            if contains_sampling || (contains_declaration && !overlaps_function) {
                self.nodes.push(SyntaxNode {
                    kind: if contains_sampling {
                        SyntaxNodeKind::SamplingStatement
                    } else {
                        SyntaxNodeKind::VariableDeclaration
                    },
                    range: TextRange {
                        start: self.tokens[start].range.start,
                        end: self.tokens[end].range.end,
                    },
                    token_range: start..end.saturating_add(1),
                    parent: Some(0),
                });
            }
        }
    }
}

fn last_source_token(tokens: &[Token], indices: &[usize]) -> Option<usize> {
    indices
        .iter()
        .copied()
        .rev()
        .find(|index| tokens[*index].kind != SyntaxKind::EndOfFile)
}

fn top_level_expression_start(tokens: &[Token], indices: &[usize]) -> Option<usize> {
    let mut parens = 0usize;
    let mut brackets = 0usize;
    let mut angles = 0usize;
    for (position, index) in indices.iter().copied().enumerate() {
        match tokens[index].kind {
            SyntaxKind::Symbol(Symbol::LeftParen) => parens += 1,
            SyntaxKind::Symbol(Symbol::RightParen) => parens = parens.saturating_sub(1),
            SyntaxKind::Symbol(Symbol::LeftBracket) => brackets += 1,
            SyntaxKind::Symbol(Symbol::RightBracket) => brackets = brackets.saturating_sub(1),
            SyntaxKind::Symbol(Symbol::LessThan) => angles += 1,
            SyntaxKind::Symbol(Symbol::GreaterThan) => angles = angles.saturating_sub(1),
            SyntaxKind::Symbol(Symbol::Assign | Symbol::Tilde)
                if parens == 0 && brackets == 0 && angles == 0 =>
            {
                return Some(position + 1);
            }
            _ => {}
        }
    }
    None
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
    fn structured_function_loop_call_and_sampling_views_have_exact_ranges() {
        let source = "functions { real f(data real x, array[2] int y) { return x; } } model { for (n in 1:2) { y ~ custom(n); } }";
        let result = parse_source(source);
        let function = result
            .tree
            .source_file()
            .function_declarations()
            .next()
            .unwrap();
        assert_eq!(function.name().unwrap().text(), "f");
        assert_eq!(function.return_type().unwrap().text().trim(), "real");
        let parameters = function.parameters();
        assert_eq!(parameters.len(), 2);
        assert!(parameters[0].is_data_only());
        assert_eq!(parameters[0].name().text(), "x");
        assert_eq!(parameters[1].name().text(), "y");
        assert_eq!(parameters[1].type_syntax().text().trim(), "array[2] int");

        let for_statement = result.tree.source_file().for_statements().next().unwrap();
        assert_eq!(for_statement.binder().unwrap().text(), "n");
        assert!(for_statement.body_range().is_some());
        let sampling = result
            .tree
            .source_file()
            .sampling_statements()
            .next()
            .unwrap();
        assert_eq!(sampling.distribution().unwrap().text(), "custom");
        assert!(sampling.is_complete());
    }

    #[test]
    fn structured_functions_preserve_container_return_types() {
        let source = "functions { array[] real f(real x) { return {x}; } tuple(real, int) g() { return (1.0, 1); } }";
        let result = parse_source(source);
        let functions = result
            .tree
            .source_file()
            .function_declarations()
            .collect::<Vec<_>>();
        assert_eq!(functions.len(), 2);
        assert_eq!(functions[0].name().unwrap().text(), "f");
        assert_eq!(
            functions[0].return_type().unwrap().text().trim(),
            "array[] real"
        );
        assert_eq!(functions[1].name().unwrap().text(), "g");
        assert_eq!(
            functions[1].return_type().unwrap().text().trim(),
            "tuple(real, int)"
        );
    }

    #[test]
    fn incomplete_calls_are_explicitly_incomplete() {
        let result = parse_source("model { normal(");
        let call = result.tree.source_file().call_expressions().next().unwrap();
        assert_eq!(call.callee().unwrap().text(), "normal");
        assert!(!call.is_complete());
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
