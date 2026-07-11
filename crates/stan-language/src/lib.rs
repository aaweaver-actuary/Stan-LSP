//! Versioned vocabulary and built-in function metadata for the Stan language.

mod analysis;
mod catalog;
mod functions;
mod lexer;
mod syntax;
mod types;

pub use catalog::{CatalogError, CatalogErrorKind, FunctionCatalog, FunctionMetadata};
pub use functions::{
    CallContext, CallContextSet, Distribution, DistributionKind, FunctionCategory, Lifecycle,
    ParseStanVersionError, StanFunction, StanVersion,
};
pub use lexer::{
    LexResult, LexicalDiagnostic, LexicalDiagnosticKind, SyntaxKind, TextRange, Token, lex,
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
pub use analysis::{Analysis, analyze};
