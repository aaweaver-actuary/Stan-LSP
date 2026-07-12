//! Structural types used by the embedded function-signature catalog.

#![allow(
    missing_docs,
    reason = "catalog data structures are exhaustively described by their field names and serialized representation"
)]

use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum StanType {
    Int,
    Real,
    Complex,
    Vector,
    RowVector,
    Matrix,
    ComplexVector,
    ComplexRowVector,
    ComplexMatrix,
    Array {
        dimensions: usize,
        element: Box<StanType>,
    },
    Tuple(Vec<StanType>),
    TypeVariable(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReturnType {
    Void,
    Value(StanType),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DataQualifier {
    AutoDiff,
    DataOnly,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Parameter {
    pub qualifier: DataQualifier,
    pub r#type: StanType,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ConcreteSignature {
    pub parameters: Vec<Parameter>,
    pub return_type: ReturnType,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CallbackSignature {
    pub parameters: Vec<Parameter>,
    pub return_type: ReturnType,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct VariadicSignature {
    pub display: String,
    pub callback: Option<CallbackSignature>,
    pub control_parameters: Vec<Parameter>,
    pub forwards_arguments: bool,
    pub return_type: ReturnType,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", content = "signature", rename_all = "snake_case")]
pub enum FunctionSignature {
    Concrete(ConcreteSignature),
    Variadic(VariadicSignature),
}

impl fmt::Display for StanType {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Int => formatter.write_str("int"),
            Self::Real => formatter.write_str("real"),
            Self::Complex => formatter.write_str("complex"),
            Self::Vector => formatter.write_str("vector"),
            Self::RowVector => formatter.write_str("row_vector"),
            Self::Matrix => formatter.write_str("matrix"),
            Self::ComplexVector => formatter.write_str("complex_vector"),
            Self::ComplexRowVector => formatter.write_str("complex_row_vector"),
            Self::ComplexMatrix => formatter.write_str("complex_matrix"),
            Self::Array {
                dimensions,
                element,
            } => {
                write!(formatter, "array[")?;
                for _ in 1..*dimensions {
                    formatter.write_str(",")?;
                }
                write!(formatter, "] {element}")
            }
            Self::Tuple(elements) => {
                formatter.write_str("tuple(")?;
                for (index, element) in elements.iter().enumerate() {
                    if index > 0 {
                        formatter.write_str(", ")?;
                    }
                    write!(formatter, "{element}")?;
                }
                formatter.write_str(")")
            }
            Self::TypeVariable(name) => formatter.write_str(name),
        }
    }
}

impl fmt::Display for ReturnType {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Void => formatter.write_str("void"),
            Self::Value(value) => value.fmt(formatter),
        }
    }
}

impl Parameter {
    pub fn display(&self) -> String {
        match self.qualifier {
            DataQualifier::AutoDiff => self.r#type.to_string(),
            DataQualifier::DataOnly => format!("data {}", self.r#type),
        }
    }
}

impl FunctionSignature {
    pub fn display(&self, name: &str) -> String {
        match self {
            Self::Concrete(signature) => format!(
                "{name}({}) => {}",
                signature
                    .parameters
                    .iter()
                    .map(Parameter::display)
                    .collect::<Vec<_>>()
                    .join(", "),
                signature.return_type
            ),
            Self::Variadic(signature) => signature.display.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn types_and_signatures_have_stable_stan_display_forms() {
        let signature = FunctionSignature::Concrete(ConcreteSignature {
            parameters: vec![Parameter {
                qualifier: DataQualifier::DataOnly,
                r#type: StanType::Array {
                    dimensions: 2,
                    element: Box::new(StanType::Real),
                },
            }],
            return_type: ReturnType::Value(StanType::Tuple(vec![StanType::Int, StanType::Real])),
        });
        assert_eq!(
            signature.display("example"),
            "example(data array[,] real) => tuple(int, real)"
        );
    }
}
