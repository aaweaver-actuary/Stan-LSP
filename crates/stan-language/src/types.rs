use serde::{Deserialize, Serialize};

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
