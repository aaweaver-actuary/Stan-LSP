//! Versioned vocabulary and built-in function metadata for the Stan language.
//!
//! The crate owns lossless lexical and syntax analysis, conservative per-file
//! semantics, shared diagnostics and lints, native formatting, and the versioned
//! built-in catalog. It does not own LSP transport, editor configuration, or
//! stanc3 process management.
//!
//! # Basic analysis
//!
//! ```
//! use stan_language::{analyze_revision, Revision};
//!
//! let analysis = analyze_revision(
//!     "parameters { real theta; } model { theta ~ normal(0, 1); }",
//!     Revision::default(),
//! );
//! assert!(analysis.syntax.source_file().program_blocks().count() == 2);
//! assert!(analysis.semantics.symbol_at(18).is_some());
//! ```

#![deny(missing_docs)]

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
    LexResult, LexicalDiagnostic, LexicalDiagnosticKind, SyntaxKind, TextRange, TextSize, Token,
    lex,
};
pub use lint::{
    ALL_LINTS, ARGUMENT_COUNT, ARGUMENT_TYPE, DEPRECATED_LANGUAGE_ELEMENT, ILLEGAL_CALL_CONTEXT,
    LintConfig, LintDescriptor, LintGroup, LintLevel, LintStability, PARAMETER_WITHOUT_PRIOR,
    REGISTRY, REPEATED_EXPENSIVE_OPERATION, UNKNOWN_DISTRIBUTION, UNRESOLVED_IDENTIFIER,
    UNUSED_DECLARATION, VECTORIZATION_OPPORTUNITY, lint,
};
pub use parser::{
    CallExpression, CompoundStatement, Declarator, ForStatement, FunctionDeclaration,
    FunctionParameter, NameRef, ParseResult, ProgramBlock, SamplingStatement, SourceFile,
    SyntaxNode, SyntaxNodeKind, SyntaxTree, TypeSyntax, VariableDeclaration, parse,
};
pub use semantics::{
    SemanticInvariantError,
    analyze_semantics::analyze_semantics,
    model::SemanticModel,
    probability_function::{ProbabilityFunctionKind, UserProbabilityFunction},
    reference::{Reference, ReferenceId},
    scope::{Scope, ScopeId, ScopeKind},
    symbol::{SemanticSymbol, SemanticSymbolKind, SymbolId},
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
    AnalysisHost, AnalysisSnapshot, FileId, Revision, analyze_revision, fallback_analysis,
};
pub use diagnostic::{
    Applicability, Diagnostic, DiagnosticCode, DiagnosticSource, Fix, RelatedDiagnostic, Severity,
    TextEdit,
};
