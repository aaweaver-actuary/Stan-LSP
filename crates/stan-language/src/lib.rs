//! Versioned vocabulary and built-in function metadata for the Stan language.

mod analysis;
mod catalog;
mod diagnostic;
mod format;
mod functions;
mod lexer;
mod lint;
mod parser;
mod semantics;
mod syntax;
mod types;

pub use catalog::{CatalogError, CatalogErrorKind, FunctionCatalog, FunctionMetadata};
pub use format::{FormatError, FormatterConfig, format, format_range};
pub use functions::{
    CallContext, CallContextSet, Distribution, DistributionKind, FunctionCategory, Lifecycle,
    ParseStanVersionError, StanFunction, StanVersion,
};
pub use lexer::{
    LexResult, LexicalDiagnostic, LexicalDiagnosticKind, SyntaxKind, TextRange, Token, lex,
};
pub use lint::{
    ALL_LINTS, ARGUMENT_COUNT, ARGUMENT_TYPE, DEPRECATED_LANGUAGE_ELEMENT, ILLEGAL_CALL_CONTEXT,
    LintConfig, LintDescriptor, LintGroup, LintLevel, PARAMETER_WITHOUT_PRIOR, REGISTRY,
    REPEATED_EXPENSIVE_OPERATION, UNKNOWN_DISTRIBUTION, UNRESOLVED_IDENTIFIER, UNUSED_DECLARATION,
    VECTORIZATION_OPPORTUNITY, lint,
};
pub use parser::{
    ParseResult, ProgramBlock, SourceFile, SyntaxNode, SyntaxNodeKind, SyntaxTree, parse,
};
pub use semantics::{
    Reference, Scope, ScopeId, SemanticModel, SemanticSymbol, SemanticSymbolKind, SymbolId,
    analyze_semantics,
};
pub use syntax::{
    Associativity, Directive, Fixity, Keyword, KeywordRole, LegacyLanguageElement, LexemeKind,
    OperatorForm, ProgramBlockKind, Symbol, SymbolRole,
};
pub use types::{
    CallbackSignature, ConcreteSignature, DataQualifier, FunctionSignature, Parameter, ReturnType,
    StanType, VariadicSignature,
};

/// The Stan language version represented by this crate's embedded catalog.
pub const STAN_VERSION: StanVersion = StanVersion::new(2, 39, 0);
pub use analysis::{
    Analysis, AnalysisHost, AnalysisSnapshot, FileId, Revision, analyze, analyze_revision,
    fallback_analysis,
};
pub use diagnostic::{
    Applicability, Diagnostic, DiagnosticCode, DiagnosticSource, Fix, RelatedDiagnostic, Severity,
    TextEdit,
};
