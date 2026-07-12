//! Deterministic native formatting over the lossless syntax tree.

use crate::{
    AnalysisSnapshot, Revision, Symbol, SyntaxKind, SyntaxNodeKind, TextEdit, TextRange,
    analyze_revision,
};

#[derive(Debug, Clone, PartialEq, Eq)]
/// Deterministic options shared by the formatter library, CLI, and LSP endpoint.
pub struct FormatterConfig {
    /// Number of spaces used for one indentation level.
    pub indent_width: usize,
}

impl Default for FormatterConfig {
    fn default() -> Self {
        Self { indent_width: 2 }
    }
}

impl FormatterConfig {
    /// Parses the supported `stanfmt.toml` keys strictly.
    ///
    /// # Errors
    ///
    /// Returns a message containing the offending line or option.
    ///
    /// ```
    /// let config = stan_language::FormatterConfig::from_toml("indent_width = 4").unwrap();
    /// assert_eq!(config.indent_width, 4);
    /// ```
    pub fn from_toml(input: &str) -> Result<Self, String> {
        let mut config = Self::default();
        for (line_number, raw) in input.lines().enumerate() {
            let line = raw.split('#').next().unwrap_or_default().trim();
            if line.is_empty() {
                continue;
            }
            let (key, value) = line
                .split_once('=')
                .ok_or_else(|| format!("stanfmt.toml line {} has no `=`", line_number + 1))?;
            match key.trim() {
                "indent_width" => {
                    config.indent_width = value.trim().parse().map_err(|_| {
                        format!(
                            "stanfmt.toml line {} has invalid indent width",
                            line_number + 1
                        )
                    })?;
                    if config.indent_width == 0 || config.indent_width > 16 {
                        return Err("indent_width must be between 1 and 16".to_owned());
                    }
                }
                other => return Err(format!("unknown stanfmt setting `{other}`")),
            }
        }
        Ok(config)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
/// Controlled reason that native formatting cannot safely proceed.
pub enum FormatError {
    #[error("source contains lexical or structural errors")]
    /// The recovery tree contains errors for which formatting is not proven safe.
    InvalidSyntax,
}

/// Formats one complete analysis snapshot deterministically.
///
/// # Errors
///
/// Returns [`FormatError::InvalidSyntax`] when the input is not safe to format.
pub fn format(
    snapshot: &AnalysisSnapshot,
    config: &FormatterConfig,
) -> Result<String, FormatError> {
    if snapshot
        .diagnostics
        .iter()
        .any(|diagnostic| matches!(diagnostic.severity, crate::Severity::Error))
    {
        return Err(FormatError::InvalidSyntax);
    }
    let source = snapshot.syntax.text();
    let mut output = String::new();
    let mut indent = 0usize;
    let mut line_start = true;
    let mut pending_space = false;

    for token in snapshot.tokens.iter() {
        let spelling = &source[token.range.start as usize..token.range.end as usize];
        match token.kind {
            SyntaxKind::EndOfFile => {}
            SyntaxKind::Whitespace => pending_space = !line_start,
            SyntaxKind::LineComment => {
                write_pending(
                    &mut output,
                    &mut line_start,
                    &mut pending_space,
                    indent,
                    config,
                );
                output.push_str(spelling.trim_end());
                newline(&mut output, &mut line_start, &mut pending_space);
            }
            SyntaxKind::BlockComment => {
                write_pending(
                    &mut output,
                    &mut line_start,
                    &mut pending_space,
                    indent,
                    config,
                );
                output.push_str(spelling);
                pending_space = true;
            }
            SyntaxKind::IncludePath => {
                write_pending(
                    &mut output,
                    &mut line_start,
                    &mut pending_space,
                    indent,
                    config,
                );
                output.push_str(spelling);
                newline(&mut output, &mut line_start, &mut pending_space);
            }
            SyntaxKind::Symbol(Symbol::LeftBrace) => {
                write_pending(
                    &mut output,
                    &mut line_start,
                    &mut pending_space,
                    indent,
                    config,
                );
                output.push('{');
                indent += 1;
                newline(&mut output, &mut line_start, &mut pending_space);
            }
            SyntaxKind::Symbol(Symbol::RightBrace) => {
                indent = indent.saturating_sub(1);
                if !line_start {
                    newline(&mut output, &mut line_start, &mut pending_space);
                }
                write_indent(&mut output, &mut line_start, indent, config);
                output.push('}');
                newline(&mut output, &mut line_start, &mut pending_space);
            }
            SyntaxKind::Symbol(Symbol::Semicolon) => {
                output.push(';');
                newline(&mut output, &mut line_start, &mut pending_space);
            }
            SyntaxKind::Symbol(Symbol::Comma) => {
                output.push(',');
                pending_space = true;
            }
            SyntaxKind::Symbol(Symbol::LeftParen | Symbol::LeftBracket) => {
                write_indent(&mut output, &mut line_start, indent, config);
                output.push_str(spelling);
                pending_space = false;
            }
            SyntaxKind::Symbol(Symbol::RightParen | Symbol::RightBracket) => {
                trim_spaces(&mut output);
                output.push_str(spelling);
                pending_space = false;
            }
            SyntaxKind::Symbol(symbol)
                if !symbol.operator_forms().is_empty() || symbol == Symbol::Tilde =>
            {
                pending_space = true;
                write_pending(
                    &mut output,
                    &mut line_start,
                    &mut pending_space,
                    indent,
                    config,
                );
                output.push_str(spelling);
                pending_space = true;
            }
            _ => {
                write_pending(
                    &mut output,
                    &mut line_start,
                    &mut pending_space,
                    indent,
                    config,
                );
                output.push_str(spelling);
                pending_space = needs_separation(token.kind);
            }
        }
    }
    while output.ends_with([' ', '\n', '\r', '\t']) {
        output.pop();
    }
    output.push('\n');
    Ok(output)
}

/// Formats the smallest complete syntax construct containing `requested`.
///
/// # Errors
///
/// Returns [`FormatError::InvalidSyntax`] when no safe construct is available.
pub fn format_range(
    snapshot: &AnalysisSnapshot,
    config: &FormatterConfig,
    requested: TextRange,
) -> Result<TextEdit, FormatError> {
    let node = snapshot
        .syntax
        .nodes()
        .iter()
        .filter(|node| {
            node.range.start <= requested.start
                && requested.end <= node.range.end
                && matches!(
                    node.kind,
                    SyntaxNodeKind::Statement
                        | SyntaxNodeKind::VariableDeclaration
                        | SyntaxNodeKind::SamplingStatement
                        | SyntaxNodeKind::ProgramBlock(_)
                        | SyntaxNodeKind::FunctionDeclaration
                )
        })
        .min_by_key(|node| node.range.end - node.range.start);
    let range = node.map_or(requested, |node| node.range);
    let source = &snapshot.syntax.text()[range.start as usize..range.end as usize];
    let replacement = format(&analyze_revision(source, Revision::default()), config)?;
    Ok(TextEdit { range, replacement })
}

fn needs_separation(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        SyntaxKind::Keyword(_)
            | SyntaxKind::Identifier
            | SyntaxKind::IntegerLiteral
            | SyntaxKind::RealLiteral
            | SyntaxKind::ImaginaryLiteral
            | SyntaxKind::StringLiteral
            | SyntaxKind::Legacy(_)
            | SyntaxKind::Directive(_)
    )
}

fn write_pending(
    output: &mut String,
    line_start: &mut bool,
    pending_space: &mut bool,
    indent: usize,
    config: &FormatterConfig,
) {
    write_indent(output, line_start, indent, config);
    if *pending_space && !output.ends_with([' ', '\n']) {
        output.push(' ');
    }
    *pending_space = false;
}

fn write_indent(
    output: &mut String,
    line_start: &mut bool,
    indent: usize,
    config: &FormatterConfig,
) {
    if *line_start {
        output.extend(std::iter::repeat_n(' ', indent * config.indent_width));
        *line_start = false;
    }
}

fn newline(output: &mut String, line_start: &mut bool, pending_space: &mut bool) {
    trim_spaces(output);
    if !output.ends_with('\n') {
        output.push('\n');
    }
    *line_start = true;
    *pending_space = false;
}

fn trim_spaces(output: &mut String) {
    while output.ends_with([' ', '\t']) {
        output.pop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{analyze_revision, format_range};

    #[test]
    fn formatting_is_deterministic_and_idempotent() {
        let source = "data{real y;}model{y~normal(0,1);}";
        let first = format(
            &analyze_revision(source, Revision::default()),
            &FormatterConfig::default(),
        )
        .unwrap();
        assert_eq!(
            first,
            "data {\n  real y;\n}\nmodel {\n  y ~ normal(0, 1);\n}\n"
        );
        let second = format(
            &analyze_revision(&first, Revision::default()),
            &FormatterConfig::default(),
        )
        .unwrap();
        assert_eq!(second, first);
    }

    #[test]
    fn formatting_preserves_comments_directives_and_strings() {
        let source = "#include shared.stan\nmodel{/* keep */print(\"a b\");// tail\n}";
        let formatted = format(
            &analyze_revision(source, Revision::default()),
            &FormatterConfig::default(),
        )
        .unwrap();
        for preserved in ["#include shared.stan", "/* keep */", "\"a b\"", "// tail"] {
            assert!(formatted.contains(preserved), "{formatted}");
        }
        assert_eq!(
            format(
                &analyze_revision(&formatted, Revision::default()),
                &FormatterConfig::default()
            )
            .unwrap(),
            formatted
        );
    }

    #[test]
    fn formatter_configuration_is_strict() {
        assert_eq!(
            FormatterConfig::from_toml("indent_width = 4")
                .unwrap()
                .indent_width,
            4
        );
        assert!(FormatterConfig::from_toml("unknown = 1").is_err());
    }

    #[test]
    fn range_formatting_selects_a_complete_syntax_construct() {
        let source = "model { real x; x=1; }";
        let snapshot = analyze_revision(source, Revision::default());
        let offset = source.find("x=1").unwrap();
        let edit = format_range(
            &snapshot,
            &FormatterConfig::default(),
            TextRange::new(offset, offset + 3),
        )
        .unwrap();
        assert_eq!(edit.replacement, "x = 1;\n");
    }
}
