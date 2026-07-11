use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

use crate::{
    CallContextSet, ConcreteSignature, DataQualifier, FunctionCategory, FunctionSignature,
    Lifecycle, Parameter, ReturnType, StanFunction, StanType,
};

const SIGNATURES: &str = include_str!("../../../catalog/stan-2.39/signatures.txt");
const SPECIAL_SIGNATURES: &str = include_str!("../../../catalog/stan-2.39/special-signatures.tsv");

#[derive(Debug, Clone)]
pub struct FunctionMetadata {
    pub categories: BTreeSet<FunctionCategory>,
    pub lifecycle: Lifecycle,
    pub call_contexts: CallContextSet,
}

#[derive(Debug)]
pub struct FunctionCatalog {
    signatures: BTreeMap<StanFunction, Vec<FunctionSignature>>,
    metadata: BTreeMap<StanFunction, FunctionMetadata>,
}

impl FunctionCatalog {
    pub fn global() -> &'static Self {
        static CATALOG: OnceLock<FunctionCatalog> = OnceLock::new();
        CATALOG.get_or_init(|| {
            Self::parse(SIGNATURES).expect("embedded Stan 2.39 signature catalog is valid")
        })
    }

    pub fn parse(input: &str) -> Result<Self, String> {
        let mut signatures = BTreeMap::<StanFunction, Vec<FunctionSignature>>::new();
        for (line_number, raw_line) in input.lines().enumerate() {
            let line = raw_line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let (name, signature) = parse_signature(line)
                .map_err(|error| format!("line {}: {error}", line_number + 1))?;
            let function = StanFunction::from_str(name)
                .map_err(|_| format!("line {}: unknown function {name:?}", line_number + 1))?;
            signatures
                .entry(function)
                .or_default()
                .push(FunctionSignature::Concrete(signature));
        }

        for (line_number, raw_line) in SPECIAL_SIGNATURES.lines().enumerate() {
            let line = raw_line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let mut fields = line.splitn(3, '\t');
            let name = fields.next().unwrap_or_default();
            let display = fields.next().ok_or_else(|| {
                format!(
                    "special signature line {} has no display form",
                    line_number + 1
                )
            })?;
            let return_type = fields.next().ok_or_else(|| {
                format!(
                    "special signature line {} has no return type",
                    line_number + 1
                )
            })?;
            let function = StanFunction::from_str(name).map_err(|_| {
                format!(
                    "special signature line {} has unknown function {name:?}",
                    line_number + 1
                )
            })?;
            let return_type = if return_type == "void" {
                ReturnType::Void
            } else {
                ReturnType::Value(
                    parse_type(return_type)
                        .unwrap_or_else(|_| StanType::TypeVariable(return_type.to_owned())),
                )
            };
            let callback_return = if name.starts_with("ode_")
                || name.starts_with("dae")
                || name.starts_with("solve_")
            {
                ReturnType::Value(StanType::Vector)
            } else {
                ReturnType::Value(StanType::Real)
            };
            let callback = crate::CallbackSignature {
                parameters: vec![Parameter {
                    qualifier: DataQualifier::AutoDiff,
                    r#type: StanType::TypeVariable("callback arguments".to_owned()),
                }],
                return_type: callback_return,
            };
            let control_parameters = if name.contains("_tol") {
                vec![Parameter {
                    qualifier: DataQualifier::DataOnly,
                    r#type: StanType::TypeVariable("tolerance controls".to_owned()),
                }]
            } else {
                Vec::new()
            };
            signatures
                .entry(function)
                .or_default()
                .push(FunctionSignature::Variadic(crate::VariadicSignature {
                    display: display.to_owned(),
                    callback: Some(callback),
                    control_parameters,
                    forwards_arguments: true,
                    return_type,
                }));
        }

        let mut metadata = BTreeMap::new();
        for function in StanFunction::ALL {
            metadata.insert(*function, infer_metadata(*function));
        }

        Ok(Self {
            signatures,
            metadata,
        })
    }

    pub fn signatures(&self, function: StanFunction) -> &[FunctionSignature] {
        self.signatures
            .get(&function)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    pub fn metadata(&self, function: StanFunction) -> &FunctionMetadata {
        &self.metadata[&function]
    }

    pub fn functions(&self) -> impl Iterator<Item = StanFunction> + '_ {
        self.signatures.keys().copied()
    }
}

impl StanFunction {
    pub fn signatures(&self) -> &'static [FunctionSignature] {
        FunctionCatalog::global().signatures(*self)
    }

    pub fn metadata(&self) -> &'static FunctionMetadata {
        FunctionCatalog::global().metadata(*self)
    }
}

fn infer_metadata(function: StanFunction) -> FunctionMetadata {
    let name = function.as_str();
    let categories = function.declared_categories().iter().copied().collect();

    let call_contexts = if name.ends_with("_rng") {
        CallContextSet::TRANSFORMED_DATA
            .union(CallContextSet::GENERATED_QUANTITIES)
            .union(CallContextSet::RNG_FUNCTION)
    } else if name.ends_with("_jacobian") {
        CallContextSet::TRANSFORMED_PARAMETERS.union(CallContextSet::JACOBIAN_FUNCTION)
    } else if name.ends_with("_lupdf") || name.ends_with("_lupmf") {
        CallContextSet::MODEL.union(CallContextSet::LOG_PROBABILITY_FUNCTION)
    } else {
        CallContextSet::ANY
    };

    let lifecycle = function.declared_lifecycle();
    FunctionMetadata {
        categories,
        lifecycle,
        call_contexts,
    }
}

fn parse_signature(line: &str) -> Result<(&str, ConcreteSignature), String> {
    let open = line
        .find('(')
        .ok_or_else(|| format!("missing argument list in {line:?}"))?;
    let close = matching_delimiter(line, open, '(', ')')?;
    let name = &line[..open];
    let tail = line[close + 1..].trim();
    let return_text = tail
        .strip_prefix("=>")
        .ok_or_else(|| format!("missing return arrow in {line:?}"))?
        .trim();
    let arguments = &line[open + 1..close];
    let parameters = if arguments.trim().is_empty() {
        Vec::new()
    } else {
        split_top_level(arguments, ',')
            .into_iter()
            .map(parse_parameter)
            .collect::<Result<Vec<_>, _>>()?
    };
    let return_type = if return_text == "void" {
        ReturnType::Void
    } else {
        ReturnType::Value(parse_type(return_text)?)
    };
    Ok((
        name,
        ConcreteSignature {
            parameters,
            return_type,
        },
    ))
}

fn parse_parameter(text: &str) -> Result<Parameter, String> {
    let text = text.trim();
    if text.starts_with('(') {
        return Ok(Parameter {
            qualifier: DataQualifier::AutoDiff,
            r#type: StanType::TypeVariable(text.to_owned()),
        });
    }
    let (qualifier, text) = match text.strip_prefix("data ") {
        Some(text) => (DataQualifier::DataOnly, text),
        None => (DataQualifier::AutoDiff, text),
    };
    Ok(Parameter {
        qualifier,
        r#type: parse_type(text)?,
    })
}

fn parse_type(text: &str) -> Result<StanType, String> {
    let text = text.trim();
    if let Some(rest) = text.strip_prefix("array[") {
        let close = rest
            .find(']')
            .ok_or_else(|| format!("unterminated array type {text:?}"))?;
        let dimensions = rest[..close]
            .chars()
            .filter(|character| *character == ',')
            .count()
            + 1;
        let element = rest[close + 1..].trim();
        if element.is_empty() {
            return Err(format!("array type has no element type: {text:?}"));
        }
        return Ok(StanType::Array {
            dimensions,
            element: Box::new(parse_type(element)?),
        });
    }
    if text.starts_with("tuple(") {
        let close = matching_delimiter(text, 5, '(', ')')?;
        if close != text.len() - 1 {
            return Err(format!("unexpected tuple suffix in {text:?}"));
        }
        let inner = &text[6..close];
        let elements = split_top_level(inner, ',')
            .into_iter()
            .map(parse_type)
            .collect::<Result<Vec<_>, _>>()?;
        return Ok(StanType::Tuple(elements));
    }
    match text {
        "int" => Ok(StanType::Int),
        "real" => Ok(StanType::Real),
        "complex" => Ok(StanType::Complex),
        "vector" => Ok(StanType::Vector),
        "row_vector" => Ok(StanType::RowVector),
        "matrix" => Ok(StanType::Matrix),
        "complex_vector" => Ok(StanType::ComplexVector),
        "complex_row_vector" => Ok(StanType::ComplexRowVector),
        "complex_matrix" => Ok(StanType::ComplexMatrix),
        _ => Err(format!("unknown Stan type {text:?}")),
    }
}

fn matching_delimiter(text: &str, open: usize, left: char, right: char) -> Result<usize, String> {
    let mut depth = 0usize;
    for (offset, character) in text[open..].char_indices() {
        if character == left {
            depth += 1;
        }
        if character == right {
            depth = depth
                .checked_sub(1)
                .ok_or_else(|| format!("unbalanced delimiter in {text:?}"))?;
            if depth == 0 {
                return Ok(open + offset);
            }
        }
    }
    Err(format!("unclosed delimiter in {text:?}"))
}

fn split_top_level(text: &str, separator: char) -> Vec<&str> {
    let mut round = 0usize;
    let mut square = 0usize;
    let mut start = 0usize;
    let mut values = Vec::new();
    for (index, character) in text.char_indices() {
        match character {
            '(' => round += 1,
            ')' => round = round.saturating_sub(1),
            '[' => square += 1,
            ']' => square = square.saturating_sub(1),
            _ if character == separator && round == 0 && square == 0 => {
                values.push(text[start..index].trim());
                start = index + character.len_utf8();
            }
            _ => {}
        }
    }
    values.push(text[start..].trim());
    values
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_nested_and_qualified_types() {
        let (_, signature) =
            parse_signature("example(data array[,] real, tuple(int, vector)) => array[] complex")
                .unwrap();
        assert_eq!(signature.parameters.len(), 2);
        assert_eq!(signature.parameters[0].qualifier, DataQualifier::DataOnly);
        assert_eq!(
            signature.return_type,
            ReturnType::Value(StanType::Array {
                dimensions: 1,
                element: Box::new(StanType::Complex),
            })
        );
    }
}
