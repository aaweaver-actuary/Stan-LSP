use stan_token_derive::Token;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Token)]
pub enum Keyword {
    #[token("functions")]
    Functions,
    #[token("data")]
    Data,
    #[token("transformed data")]
    TransformedData,
    #[token("parameters")]
    Parameters,
    #[token("transformed parameters")]
    TransformedParameters,
    #[token("model")]
    Model,
    #[token("generated quantities")]
    GeneratedQuantities,
    #[token("return")]
    Return,
    #[token("if")]
    If,
    #[token("else")]
    Else,
    #[token("while")]
    While,
    #[token("profile")]
    Profile,
    #[token("for")]
    For,
    #[token("in")]
    In,
    #[token("break")]
    Break,
    #[token("continue")]
    Continue,
    #[token("void")]
    Void,
    #[token("int")]
    Int,
    #[token("real")]
    Real,
    #[token("complex")]
    Complex,
    #[token("vector")]
    Vector,
    #[token("row_vector")]
    RowVector,
    #[token("complex_vector")]
    ComplexVector,
    #[token("complex_row_vector")]
    ComplexRowVector,
    #[token("tuple")]
    Tuple,
    #[token("array")]
    Array,
    #[token("matrix")]
    Matrix,
    #[token("complex_matrix")]
    ComplexMatrix,
    #[token("ordered")]
    Ordered,
    #[token("positive_ordered")]
    PositiveOrdered,
    #[token("simplex")]
    Simplex,
    #[token("unit_vector")]
    UnitVector,
    #[token("sum_to_zero_vector")]
    SumToZeroVector,
    #[token("sum_to_zero_matrix")]
    SumToZeroMatrix,
    #[token("cholesky_factor_corr")]
    CholeskyFactorCorr,
    #[token("cholesky_factor_cov")]
    CholeskyFactorCov,
    #[token("corr_matrix")]
    CorrMatrix,
    #[token("cov_matrix")]
    CovMatrix,
    #[token("column_stochastic_matrix")]
    ColumnStochasticMatrix,
    #[token("row_stochastic_matrix")]
    RowStochasticMatrix,
    #[token("lower")]
    Lower,
    #[token("upper")]
    Upper,
    #[token("offset")]
    Offset,
    #[token("multiplier")]
    Multiplier,
    #[token("jacobian")]
    Jacobian,
    #[token("print")]
    Print,
    #[token("reject")]
    Reject,
    #[token("fatal_error")]
    FatalError,
    #[token("target")]
    Target,
    #[token("T")]
    Truncation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KeywordRole {
    ProgramBlock,
    ControlFlow,
    Type,
    Constraint,
    Statement,
    Qualifier,
    SpecialIdentifier,
}

impl Keyword {
    pub const fn roles(&self) -> &'static [KeywordRole] {
        use KeywordRole::*;
        match self {
            Self::Functions
            | Self::TransformedData
            | Self::Parameters
            | Self::TransformedParameters
            | Self::Model
            | Self::GeneratedQuantities => &[ProgramBlock],
            Self::Data => &[ProgramBlock, Qualifier],
            Self::Return
            | Self::If
            | Self::Else
            | Self::While
            | Self::Profile
            | Self::For
            | Self::In
            | Self::Break
            | Self::Continue => &[ControlFlow],
            Self::Void
            | Self::Int
            | Self::Real
            | Self::Complex
            | Self::Vector
            | Self::RowVector
            | Self::ComplexVector
            | Self::ComplexRowVector
            | Self::Tuple
            | Self::Array
            | Self::Matrix
            | Self::ComplexMatrix
            | Self::Ordered
            | Self::PositiveOrdered
            | Self::Simplex
            | Self::UnitVector
            | Self::SumToZeroVector
            | Self::SumToZeroMatrix
            | Self::CholeskyFactorCorr
            | Self::CholeskyFactorCov
            | Self::CorrMatrix
            | Self::CovMatrix
            | Self::ColumnStochasticMatrix
            | Self::RowStochasticMatrix => &[Type],
            Self::Lower | Self::Upper | Self::Offset | Self::Multiplier => &[Constraint],
            Self::Jacobian | Self::Target => &[SpecialIdentifier],
            Self::Print | Self::Reject | Self::FatalError => &[Statement],
            Self::Truncation => &[SpecialIdentifier],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Token)]
pub enum Symbol {
    #[token("{")]
    LeftBrace,
    #[token("}")]
    RightBrace,
    #[token("(")]
    LeftParen,
    #[token(")")]
    RightParen,
    #[token("[")]
    LeftBracket,
    #[token("]")]
    RightBracket,
    #[token("<")]
    LessThan,
    #[token(">")]
    GreaterThan,
    #[token(",")]
    Comma,
    #[token(";")]
    Semicolon,
    #[token("|")]
    Bar,
    #[token("?")]
    Question,
    #[token(":")]
    Colon,
    #[token("!")]
    Bang,
    #[token("-")]
    Minus,
    #[token("+")]
    Plus,
    #[token("^")]
    Power,
    #[token("'")]
    Transpose,
    #[token("*")]
    Times,
    #[token("/")]
    Divide,
    #[token("%")]
    Modulo,
    #[token("%/%")]
    IntegerDivide,
    #[token("\\")]
    LeftDivide,
    #[token(".*")]
    ElementwiseTimes,
    #[token(".^")]
    ElementwisePower,
    #[token("./")]
    ElementwiseDivide,
    #[token("||")]
    Or,
    #[token("&&")]
    And,
    #[token("==")]
    Equal,
    #[token("!=")]
    NotEqual,
    #[token("<=")]
    LessThanOrEqual,
    #[token(">=")]
    GreaterThanOrEqual,
    #[token("~")]
    Tilde,
    #[token("=")]
    Assign,
    #[token("+=")]
    PlusAssign,
    #[token("-=")]
    MinusAssign,
    #[token("*=")]
    TimesAssign,
    #[token("/=")]
    DivideAssign,
    #[token(".*=")]
    ElementwiseTimesAssign,
    #[token("./=")]
    ElementwiseDivideAssign,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SymbolRole {
    Delimiter,
    Separator,
    Arithmetic,
    Logical,
    Assignment,
    Sampling,
    Truncation,
    Indexing,
    Conditional,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Fixity {
    Prefix,
    Infix,
    Postfix,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Associativity {
    Left,
    Right,
    NonAssociative,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct OperatorForm {
    pub fixity: Fixity,
    pub precedence: u8,
    pub associativity: Associativity,
}

impl Symbol {
    pub const fn roles(&self) -> &'static [SymbolRole] {
        use SymbolRole::*;
        match self {
            Self::LeftBrace | Self::RightBrace | Self::LeftParen | Self::RightParen => &[Delimiter],
            Self::LeftBracket | Self::RightBracket => &[Delimiter, Indexing, Truncation],
            Self::LessThan | Self::GreaterThan => &[Delimiter, Logical],
            Self::Comma | Self::Semicolon | Self::Bar => &[Separator],
            Self::Question => &[Conditional],
            Self::Colon => &[Separator, Indexing, Conditional],
            Self::Bang
            | Self::Or
            | Self::And
            | Self::Equal
            | Self::NotEqual
            | Self::LessThanOrEqual
            | Self::GreaterThanOrEqual => &[Logical],
            Self::Minus
            | Self::Plus
            | Self::Power
            | Self::Transpose
            | Self::Times
            | Self::Divide
            | Self::Modulo
            | Self::IntegerDivide
            | Self::LeftDivide
            | Self::ElementwiseTimes
            | Self::ElementwisePower
            | Self::ElementwiseDivide => &[Arithmetic],
            Self::Tilde => &[Sampling],
            Self::Assign
            | Self::PlusAssign
            | Self::MinusAssign
            | Self::TimesAssign
            | Self::DivideAssign
            | Self::ElementwiseTimesAssign
            | Self::ElementwiseDivideAssign => &[Assignment],
        }
    }

    /// Operator precedence levels, with larger values binding more tightly.
    pub const fn operator_forms(&self) -> &'static [OperatorForm] {
        use Associativity::{Left, NonAssociative, Right};
        use Fixity::{Infix, Postfix, Prefix};
        match self {
            Self::Transpose => &[OperatorForm {
                fixity: Postfix,
                precedence: 12,
                associativity: Left,
            }],
            Self::Power | Self::ElementwisePower => &[OperatorForm {
                fixity: Infix,
                precedence: 11,
                associativity: Right,
            }],
            Self::Bang => &[OperatorForm {
                fixity: Prefix,
                precedence: 10,
                associativity: Right,
            }],
            Self::Minus | Self::Plus => &[
                OperatorForm {
                    fixity: Prefix,
                    precedence: 10,
                    associativity: Right,
                },
                OperatorForm {
                    fixity: Infix,
                    precedence: 7,
                    associativity: Left,
                },
            ],
            Self::Times
            | Self::Divide
            | Self::Modulo
            | Self::IntegerDivide
            | Self::LeftDivide
            | Self::ElementwiseTimes
            | Self::ElementwiseDivide => &[OperatorForm {
                fixity: Infix,
                precedence: 8,
                associativity: Left,
            }],
            Self::LessThan
            | Self::LessThanOrEqual
            | Self::GreaterThan
            | Self::GreaterThanOrEqual
            | Self::Equal
            | Self::NotEqual => &[OperatorForm {
                fixity: Infix,
                precedence: 6,
                associativity: NonAssociative,
            }],
            Self::And => &[OperatorForm {
                fixity: Infix,
                precedence: 5,
                associativity: Left,
            }],
            Self::Or => &[OperatorForm {
                fixity: Infix,
                precedence: 4,
                associativity: Left,
            }],
            Self::Question => &[OperatorForm {
                fixity: Infix,
                precedence: 3,
                associativity: Right,
            }],
            Self::Assign
            | Self::PlusAssign
            | Self::MinusAssign
            | Self::TimesAssign
            | Self::DivideAssign
            | Self::ElementwiseTimesAssign
            | Self::ElementwiseDivideAssign => &[OperatorForm {
                fixity: Infix,
                precedence: 1,
                associativity: Right,
            }],
            _ => &[],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Token)]
pub enum Directive {
    #[token("#include")]
    Include,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Token)]
pub enum LegacyLanguageElement {
    #[token("<-")]
    ArrowAssignment,
    #[token("#")]
    HashComment,
    #[token("increment_log_prob")]
    IncrementLogProb,
    #[token("get_lp")]
    GetLp,
    #[token("if_else")]
    IfElse,
    #[token("lp__")]
    LpInternal,
    #[token("cdf_log")]
    CdfLogSuffix,
    #[token("ccdf_log")]
    CcdfLogSuffix,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LexemeKind {
    Identifier,
    IntegerLiteral,
    RealLiteral,
    ImaginaryLiteral,
    StringLiteral,
    LineComment,
    BlockComment,
    Whitespace,
    IncludePath,
    EndOfFile,
}
