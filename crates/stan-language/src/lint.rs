//! Registry-driven Stan linting.

use std::collections::BTreeMap;

use crate::{
    AnalysisSnapshot, Applicability, CallContext, Diagnostic, DiagnosticSource, Fix,
    LegacyLanguageElement, SemanticSymbolKind, Severity, SyntaxKind, SyntaxNodeKind, TextEdit,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
/// Configurable emission level for one lint rule.
#[allow(
    missing_docs,
    reason = "variants define the complete documented lint-level scale"
)]
pub enum LintLevel {
    Allow,
    Hint,
    Warn,
    Deny,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
/// User-facing family of related lint rules.
#[allow(missing_docs, reason = "variants are the documented lint groups")]
pub enum LintGroup {
    Correctness,
    Suspicious,
    Style,
    Performance,
    Deprecated,
    Bayesian,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
/// Compatibility promise attached to a published lint ID.
#[allow(missing_docs, reason = "variants define the complete stability scale")]
pub enum LintStability {
    Stable,
    Experimental,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Registry metadata for one configurable lint.
pub struct LintDescriptor {
    /// Stable configuration and diagnostic ID.
    pub id: &'static str,
    /// Rule family.
    pub group: LintGroup,
    /// Level used when configuration has no override.
    pub default_level: LintLevel,
    /// Concise user-facing explanation.
    pub description: &'static str,
    /// Published compatibility classification.
    pub stability: LintStability,
}

/// Warns about legacy or removed language elements.
pub const DEPRECATED_LANGUAGE_ELEMENT: LintDescriptor = LintDescriptor {
    id: "deprecated.language-element",
    group: LintGroup::Deprecated,
    default_level: LintLevel::Warn,
    description: "use of syntax removed or deprecated by the selected Stan version",
    stability: LintStability::Stable,
};
/// Reports references not covered by the conservative local resolver.
pub const UNRESOLVED_IDENTIFIER: LintDescriptor = LintDescriptor {
    id: "correctness.unresolved-identifier",
    group: LintGroup::Correctness,
    default_level: LintLevel::Hint,
    description: "reference does not resolve to a declaration or built-in",
    stability: LintStability::Experimental,
};
/// Reports declarations without resolved references.
pub const UNUSED_DECLARATION: LintDescriptor = LintDescriptor {
    id: "suspicious.unused-declaration",
    group: LintGroup::Suspicious,
    default_level: LintLevel::Hint,
    description: "declaration is never referenced",
    stability: LintStability::Experimental,
};
/// Reports cataloged functions outside a known legal context.
pub const ILLEGAL_CALL_CONTEXT: LintDescriptor = LintDescriptor {
    id: "correctness.illegal-call-context",
    group: LintGroup::Correctness,
    default_level: LintLevel::Hint,
    description: "built-in function is not legal in this Stan context",
    stability: LintStability::Experimental,
};
/// Reports complete calls whose arity matches no concrete overload.
pub const ARGUMENT_COUNT: LintDescriptor = LintDescriptor {
    id: "correctness.argument-count",
    group: LintGroup::Correctness,
    default_level: LintLevel::Deny,
    description: "call argument count matches no known overload",
    stability: LintStability::Stable,
};
/// Reports complete calls whose fully known argument types match no overload.
pub const ARGUMENT_TYPE: LintDescriptor = LintDescriptor {
    id: "correctness.argument-type",
    group: LintGroup::Correctness,
    default_level: LintLevel::Hint,
    description: "call arguments match no known overload when their types are known",
    stability: LintStability::Experimental,
};
/// Reports complete sampling statements without a built-in or user distribution.
pub const UNKNOWN_DISTRIBUTION: LintDescriptor = LintDescriptor {
    id: "correctness.unknown-distribution",
    group: LintGroup::Correctness,
    default_level: LintLevel::Hint,
    description: "sampling statement names no known distribution",
    stability: LintStability::Experimental,
};
/// Hints when selected expensive operations occur inside a loop.
pub const REPEATED_EXPENSIVE_OPERATION: LintDescriptor = LintDescriptor {
    id: "performance.repeated-expensive-operation",
    group: LintGroup::Performance,
    default_level: LintLevel::Hint,
    description: "expensive matrix operation is repeated inside a loop",
    stability: LintStability::Experimental,
};
/// Hints when a sampling loop may have a vectorized representation.
pub const VECTORIZATION_OPPORTUNITY: LintDescriptor = LintDescriptor {
    id: "performance.vectorization-opportunity",
    group: LintGroup::Performance,
    default_level: LintLevel::Hint,
    description: "sampling loop may admit a vectorized form",
    stability: LintStability::Experimental,
};
/// Opt-in heuristic for parameters without an apparent prior contribution.
pub const PARAMETER_WITHOUT_PRIOR: LintDescriptor = LintDescriptor {
    id: "bayesian.parameter-without-apparent-prior",
    group: LintGroup::Bayesian,
    default_level: LintLevel::Allow,
    description: "parameter has no apparent sampling contribution",
    stability: LintStability::Experimental,
};

/// Declaration-ordered registry of every configurable lint.
pub const ALL_LINTS: &[LintDescriptor] = &[
    DEPRECATED_LANGUAGE_ELEMENT,
    UNRESOLVED_IDENTIFIER,
    UNUSED_DECLARATION,
    ILLEGAL_CALL_CONTEXT,
    ARGUMENT_COUNT,
    ARGUMENT_TYPE,
    UNKNOWN_DISTRIBUTION,
    REPEATED_EXPENSIVE_OPERATION,
    VECTORIZATION_OPPORTUNITY,
    PARAMETER_WITHOUT_PRIOR,
];
/// Compatibility alias for [`ALL_LINTS`].
pub const REGISTRY: &[LintDescriptor] = ALL_LINTS;

#[derive(Debug, Clone, Default)]
/// Per-rule lint level overrides shared by LSP and `stanlint`.
pub struct LintConfig {
    levels: BTreeMap<&'static str, LintLevel>,
}

impl LintConfig {
    /// Overrides a registry lint by its static ID.
    pub fn set(&mut self, id: &'static str, level: LintLevel) {
        self.levels.insert(id, level);
    }

    /// Returns the effective level for `descriptor`.
    pub fn level(&self, descriptor: LintDescriptor) -> LintLevel {
        self.levels
            .get(descriptor.id)
            .copied()
            .unwrap_or(descriptor.default_level)
    }

    /// Overrides a lint by a runtime ID after validating it against the registry.
    ///
    /// # Errors
    ///
    /// Returns an error when `id` is not published.
    pub fn set_named(&mut self, id: &str, level: LintLevel) -> Result<(), String> {
        let descriptor = ALL_LINTS
            .iter()
            .find(|descriptor| descriptor.id == id)
            .ok_or_else(|| format!("unknown lint `{id}`"))?;
        self.set(descriptor.id, level);
        Ok(())
    }

    /// Parses strict `stanlint.toml` rule assignments.
    ///
    /// # Errors
    ///
    /// Returns the offending line, lint ID, or level.
    ///
    /// ```
    /// let config = stan_language::LintConfig::from_toml(
    ///     "correctness.unresolved-identifier = \"allow\"",
    /// ).unwrap();
    /// assert_eq!(
    ///     config.level(stan_language::UNRESOLVED_IDENTIFIER),
    ///     stan_language::LintLevel::Allow,
    /// );
    /// ```
    pub fn from_toml(input: &str) -> Result<Self, String> {
        let mut config = Self::default();
        for (line_number, raw) in input.lines().enumerate() {
            let line = raw.split('#').next().unwrap_or_default().trim();
            if line.is_empty() || line.starts_with('[') {
                continue;
            }
            let (id, value) = line
                .split_once('=')
                .ok_or_else(|| format!("stanlint.toml line {} has no `=`", line_number + 1))?;
            let id = id.trim();
            let descriptor = ALL_LINTS
                .iter()
                .find(|descriptor| descriptor.id == id)
                .ok_or_else(|| format!("unknown lint `{id}`"))?;
            let level = match value.trim().trim_matches('"') {
                "allow" => LintLevel::Allow,
                "hint" => LintLevel::Hint,
                "warn" => LintLevel::Warn,
                "deny" => LintLevel::Deny,
                other => return Err(format!("unknown lint level `{other}`")),
            };
            config.set(descriptor.id, level);
        }
        Ok(config)
    }
}

/// Runs every enabled lint against one immutable analysis snapshot.
pub fn lint(snapshot: &AnalysisSnapshot, config: &LintConfig) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    emit_deprecated(snapshot, config, &mut diagnostics);
    emit_unresolved(snapshot, config, &mut diagnostics);
    emit_unused(snapshot, config, &mut diagnostics);
    emit_illegal_context(snapshot, config, &mut diagnostics);
    emit_argument_count(snapshot, config, &mut diagnostics);
    emit_argument_type(snapshot, config, &mut diagnostics);
    emit_unknown_distribution(snapshot, config, &mut diagnostics);
    emit_performance(snapshot, config, &mut diagnostics);
    emit_missing_prior(snapshot, config, &mut diagnostics);
    apply_suppressions(snapshot, &mut diagnostics);
    diagnostics
}

fn apply_suppressions(snapshot: &AnalysisSnapshot, diagnostics: &mut Vec<Diagnostic>) {
    let text = snapshot.syntax.text();
    let mut suppressions = Vec::<(&str, std::ops::Range<u32>)>::new();
    let mut offset = 0usize;
    let lines = text.split_inclusive('\n').collect::<Vec<_>>();
    for (index, line) in lines.iter().enumerate() {
        if let Some((_, directive)) = line.split_once("stanlint: allow") {
            let id = directive.trim();
            let next_start = offset + line.len();
            let next_end = lines
                .get(index + 1)
                .map_or(next_start, |next| next_start + next.len());
            suppressions.push((id, next_start as u32..next_end as u32));
        }
        offset += line.len();
    }
    diagnostics.retain(|diagnostic| {
        !suppressions.iter().any(|(id, range)| {
            (*id == "all" || *id == diagnostic.code.0)
                && range.contains(&diagnostic.primary_range.start)
        })
    });
}

fn emit_deprecated(snapshot: &AnalysisSnapshot, config: &LintConfig, output: &mut Vec<Diagnostic>) {
    let level = config.level(DEPRECATED_LANGUAGE_ELEMENT);
    if level == LintLevel::Allow {
        return;
    }
    for token in snapshot.tokens.iter() {
        let SyntaxKind::Legacy(element) = token.kind else {
            continue;
        };
        output.push(Diagnostic {
            code: crate::DiagnosticCode(DEPRECATED_LANGUAGE_ELEMENT.id),
            severity: severity(level),
            message: format!("`{}` is a legacy Stan language element", element.as_str()),
            primary_range: token.range,
            related: Vec::new(),
            fixes: legacy_fix(element, token.range).into_iter().collect(),
            source: DiagnosticSource::StanLint,
        });
    }
}

fn emit_unresolved(snapshot: &AnalysisSnapshot, config: &LintConfig, output: &mut Vec<Diagnostic>) {
    let level = config.level(UNRESOLVED_IDENTIFIER);
    if level == LintLevel::Allow {
        return;
    }
    output.extend(
        snapshot
            .semantics
            .references
            .iter()
            .filter(|reference| reference.resolved.is_none())
            .map(|reference| Diagnostic {
                code: crate::DiagnosticCode(UNRESOLVED_IDENTIFIER.id),
                severity: severity(level),
                message: format!("cannot resolve `{}`", reference.name),
                primary_range: reference.range,
                related: Vec::new(),
                fixes: Vec::new(),
                source: DiagnosticSource::StanLint,
            }),
    );
}

fn emit_unused(snapshot: &AnalysisSnapshot, config: &LintConfig, output: &mut Vec<Diagnostic>) {
    let level = config.level(UNUSED_DECLARATION);
    if level == LintLevel::Allow {
        return;
    }
    output.extend(snapshot.semantics.symbols.iter().filter_map(|symbol| {
        let used = snapshot
            .semantics
            .references
            .iter()
            .any(|reference| reference.resolved == Some(symbol.id));
        (!used).then(|| Diagnostic {
            code: crate::DiagnosticCode(UNUSED_DECLARATION.id),
            severity: severity(level),
            message: format!("`{}` is never used", symbol.name),
            primary_range: symbol.name_range,
            related: Vec::new(),
            fixes: Vec::new(),
            source: DiagnosticSource::StanLint,
        })
    }));
}

fn emit_illegal_context(
    snapshot: &AnalysisSnapshot,
    config: &LintConfig,
    output: &mut Vec<Diagnostic>,
) {
    let level = config.level(ILLEGAL_CALL_CONTEXT);
    if level == LintLevel::Allow {
        return;
    }
    let text = snapshot.syntax.text();
    for node in snapshot.syntax.nodes().iter().filter(|node| {
        node.kind == SyntaxNodeKind::FunctionCall && complete_call(snapshot, node.range)
    }) {
        let Some(token) = snapshot.tokens.get(node.token_range.start) else {
            continue;
        };
        let name = &text[token.range.start as usize..token.range.end as usize];
        let Ok(function) = crate::StanFunction::from_str(name) else {
            continue;
        };
        let Some(context) = context_at(snapshot, token.range.start) else {
            continue;
        };
        if !function.metadata().call_contexts.contains(context) {
            output.push(Diagnostic {
                code: crate::DiagnosticCode(ILLEGAL_CALL_CONTEXT.id),
                severity: severity(level),
                message: format!("`{name}` is not legal in this program block"),
                primary_range: token.range,
                related: Vec::new(),
                fixes: Vec::new(),
                source: DiagnosticSource::StanLint,
            });
        }
    }
}

fn emit_missing_prior(
    snapshot: &AnalysisSnapshot,
    config: &LintConfig,
    output: &mut Vec<Diagnostic>,
) {
    let level = config.level(PARAMETER_WITHOUT_PRIOR);
    if level == LintLevel::Allow {
        return;
    }
    let text = snapshot.syntax.text();
    for symbol in snapshot
        .semantics
        .symbols
        .iter()
        .filter(|symbol| symbol.kind == SemanticSymbolKind::Parameter)
    {
        let apparent_prior = snapshot.syntax.nodes().iter().any(|node| {
            node.kind == SyntaxNodeKind::SamplingStatement
                && text[node.range.start as usize..node.range.end as usize]
                    .split('~')
                    .next()
                    .is_some_and(|left| left.split_whitespace().any(|word| word == symbol.name))
        });
        if !apparent_prior {
            output.push(Diagnostic {
                code: crate::DiagnosticCode(PARAMETER_WITHOUT_PRIOR.id),
                severity: severity(level),
                message: format!(
                    "parameter `{}` has no apparent prior contribution",
                    symbol.name
                ),
                primary_range: symbol.name_range,
                related: Vec::new(),
                fixes: Vec::new(),
                source: DiagnosticSource::StanLint,
            });
        }
    }
}

fn emit_argument_count(
    snapshot: &AnalysisSnapshot,
    config: &LintConfig,
    output: &mut Vec<Diagnostic>,
) {
    let level = config.level(ARGUMENT_COUNT);
    if level == LintLevel::Allow {
        return;
    }
    let text = snapshot.syntax.text();
    for node in snapshot.syntax.nodes().iter().filter(|node| {
        node.kind == SyntaxNodeKind::FunctionCall && complete_call(snapshot, node.range)
    }) {
        let Some(name_token) = snapshot.tokens.get(node.token_range.start) else {
            continue;
        };
        let name = &text[name_token.range.start as usize..name_token.range.end as usize];
        let (function, sampling_distribution) = if let Ok(function) =
            crate::StanFunction::from_str(name)
        {
            (function, false)
        } else if let Ok(distribution) = crate::Distribution::from_str(name) {
            let suffix = match distribution.kind() {
                crate::DistributionKind::Continuous => "lpdf",
                crate::DistributionKind::Discrete => "lpmf",
            };
            let Ok(function) = crate::StanFunction::from_str(&format!("{name}_{suffix}")) else {
                continue;
            };
            (function, true)
        } else {
            continue;
        };
        let arguments = call_argument_count(&snapshot.tokens[node.token_range.clone()]);
        let concrete = function
            .signatures()
            .iter()
            .filter_map(|signature| match signature {
                crate::FunctionSignature::Concrete(signature) => Some(signature.parameters.len()),
                crate::FunctionSignature::Variadic(_) => None,
            })
            .map(|count| count.saturating_sub(usize::from(sampling_distribution)))
            .collect::<Vec<_>>();
        if !concrete.is_empty() && !concrete.contains(&arguments) {
            output.push(Diagnostic {
                code: crate::DiagnosticCode(ARGUMENT_COUNT.id),
                severity: severity(level),
                message: format!(
                    "`{name}` accepts {} argument count(s), but this call has {arguments}",
                    concrete
                        .iter()
                        .copied()
                        .collect::<std::collections::BTreeSet<_>>()
                        .into_iter()
                        .map(|count| count.to_string())
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
                primary_range: node.range,
                related: Vec::new(),
                fixes: Vec::new(),
                source: DiagnosticSource::StanLint,
            });
        }
    }
}

fn call_argument_count(tokens: &[crate::Token]) -> usize {
    let mut depth = 0usize;
    let mut commas = 0usize;
    let mut has_value = false;
    for token in tokens.iter().skip(1) {
        match token.kind {
            SyntaxKind::Symbol(crate::Symbol::LeftParen) => depth += 1,
            SyntaxKind::Symbol(crate::Symbol::RightParen) => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    break;
                }
            }
            SyntaxKind::Symbol(crate::Symbol::Comma) if depth == 1 => commas += 1,
            SyntaxKind::Whitespace | SyntaxKind::LineComment | SyntaxKind::BlockComment => {}
            _ if depth >= 1 => has_value = true,
            _ => {}
        }
    }
    usize::from(has_value) + commas
}

fn emit_argument_type(
    snapshot: &AnalysisSnapshot,
    config: &LintConfig,
    output: &mut Vec<Diagnostic>,
) {
    let level = config.level(ARGUMENT_TYPE);
    if level == LintLevel::Allow {
        return;
    }
    let text = snapshot.syntax.text();
    for node in snapshot.syntax.nodes().iter().filter(|node| {
        node.kind == SyntaxNodeKind::FunctionCall && complete_call(snapshot, node.range)
    }) {
        let Some(name_token) = snapshot.tokens.get(node.token_range.start) else {
            continue;
        };
        let name = &text[name_token.range.start as usize..name_token.range.end as usize];
        let (function, skip_outcome) = if let Ok(function) = crate::StanFunction::from_str(name) {
            (function, false)
        } else if let Ok(distribution) = crate::Distribution::from_str(name) {
            let suffix = match distribution.kind() {
                crate::DistributionKind::Continuous => "lpdf",
                crate::DistributionKind::Discrete => "lpmf",
            };
            let Ok(function) = crate::StanFunction::from_str(&format!("{name}_{suffix}")) else {
                continue;
            };
            (function, true)
        } else {
            continue;
        };
        let arguments = inferred_arguments(snapshot, &snapshot.tokens[node.token_range.clone()]);
        if arguments.iter().any(Option::is_none) {
            continue;
        }
        let arguments = arguments.into_iter().flatten().collect::<Vec<_>>();
        let matching_arity = function
            .signatures()
            .iter()
            .filter_map(|signature| match signature {
                crate::FunctionSignature::Concrete(signature) => {
                    let parameters = if skip_outcome {
                        signature.parameters.get(1..).unwrap_or_default()
                    } else {
                        &signature.parameters
                    };
                    (parameters.len() == arguments.len()).then_some(parameters)
                }
                crate::FunctionSignature::Variadic(_) => None,
            });
        let mut saw_matching_arity = false;
        let mut compatible = false;
        for parameters in matching_arity {
            saw_matching_arity = true;
            if parameters
                .iter()
                .zip(&arguments)
                .all(|(parameter, argument)| type_compatible(argument, &parameter.r#type))
            {
                compatible = true;
                break;
            }
        }
        if saw_matching_arity && !compatible {
            output.push(Diagnostic {
                code: crate::DiagnosticCode(ARGUMENT_TYPE.id),
                severity: severity(level),
                message: format!("known argument types match no `{name}` overload"),
                primary_range: node.range,
                related: Vec::new(),
                fixes: Vec::new(),
                source: DiagnosticSource::StanLint,
            });
        }
    }
}

fn inferred_arguments(
    snapshot: &AnalysisSnapshot,
    tokens: &[crate::Token],
) -> Vec<Option<crate::StanType>> {
    let mut arguments = Vec::new();
    let mut depth = 0usize;
    let mut current = None;
    for token in tokens.iter().skip(1) {
        match token.kind {
            SyntaxKind::Symbol(crate::Symbol::LeftParen) => depth += 1,
            SyntaxKind::Symbol(crate::Symbol::RightParen) if depth == 1 => {
                if current.is_some() || !arguments.is_empty() {
                    arguments.push(current);
                }
                break;
            }
            SyntaxKind::Symbol(crate::Symbol::RightParen) => depth = depth.saturating_sub(1),
            SyntaxKind::Symbol(crate::Symbol::Comma) if depth == 1 => {
                arguments.push(current.take());
            }
            SyntaxKind::IntegerLiteral if depth == 1 && current.is_none() => {
                current = Some(crate::StanType::Int);
            }
            SyntaxKind::RealLiteral if depth == 1 && current.is_none() => {
                current = Some(crate::StanType::Real);
            }
            SyntaxKind::ImaginaryLiteral if depth == 1 && current.is_none() => {
                current = Some(crate::StanType::Complex);
            }
            SyntaxKind::Identifier if depth == 1 && current.is_none() => {
                current = snapshot
                    .semantics
                    .references
                    .iter()
                    .find(|reference| reference.range == token.range)
                    .and_then(|reference| reference.resolved)
                    .and_then(|id| {
                        snapshot
                            .semantics
                            .symbols
                            .iter()
                            .find(|symbol| symbol.id == id)
                    })
                    .and_then(|symbol| symbol.declared_type.clone());
            }
            _ => {}
        }
    }
    arguments
}

fn type_compatible(argument: &crate::StanType, parameter: &crate::StanType) -> bool {
    argument == parameter
        || matches!(
            (argument, parameter),
            (
                crate::StanType::Int,
                crate::StanType::Real | crate::StanType::Complex
            ) | (crate::StanType::Real, crate::StanType::Complex)
                | (_, crate::StanType::TypeVariable(_))
        )
}

fn emit_unknown_distribution(
    snapshot: &AnalysisSnapshot,
    config: &LintConfig,
    output: &mut Vec<Diagnostic>,
) {
    let level = config.level(UNKNOWN_DISTRIBUTION);
    if level == LintLevel::Allow {
        return;
    }
    for node in snapshot.syntax.nodes().iter().filter(|node| {
        node.kind == SyntaxNodeKind::SamplingStatement && complete_sampling(snapshot, node.range)
    }) {
        let tokens = &snapshot.tokens[node.token_range.clone()];
        let distribution = tokens
            .iter()
            .skip_while(|token| token.kind != SyntaxKind::Symbol(crate::Symbol::Tilde))
            .skip(1)
            .find(|token| {
                !matches!(
                    token.kind,
                    SyntaxKind::Whitespace | SyntaxKind::LineComment | SyntaxKind::BlockComment
                )
            });
        let Some(token) = distribution.filter(|token| token.kind == SyntaxKind::Identifier) else {
            continue;
        };
        let name = &snapshot.syntax.text()[token.range.start as usize..token.range.end as usize];
        if crate::Distribution::from_str(name).is_err()
            && snapshot
                .semantics
                .probability_functions(name)
                .next()
                .is_none()
        {
            output.push(Diagnostic {
                code: crate::DiagnosticCode(UNKNOWN_DISTRIBUTION.id),
                severity: severity(level),
                message: format!("unknown distribution `{name}`"),
                primary_range: token.range,
                related: Vec::new(),
                fixes: Vec::new(),
                source: DiagnosticSource::StanLint,
            });
        }
    }
}

fn emit_performance(
    snapshot: &AnalysisSnapshot,
    config: &LintConfig,
    output: &mut Vec<Diagnostic>,
) {
    let expensive_level = config.level(REPEATED_EXPENSIVE_OPERATION);
    let vector_level = config.level(VECTORIZATION_OPPORTUNITY);
    let text = snapshot.syntax.text();
    for loop_node in snapshot
        .syntax
        .nodes()
        .iter()
        .filter(|node| node.kind == SyntaxNodeKind::ForStatement)
    {
        if expensive_level != LintLevel::Allow {
            for call in snapshot.syntax.nodes().iter().filter(|node| {
                node.kind == SyntaxNodeKind::FunctionCall
                    && loop_node.range.start <= node.range.start
                    && node.range.end <= loop_node.range.end
            }) {
                let token = &snapshot.tokens[call.token_range.start];
                let name = &text[token.range.start as usize..token.range.end as usize];
                if matches!(
                    name,
                    "inverse"
                        | "determinant"
                        | "log_determinant"
                        | "cholesky_decompose"
                        | "eigendecompose"
                ) {
                    output.push(Diagnostic {
                        code: crate::DiagnosticCode(REPEATED_EXPENSIVE_OPERATION.id),
                        severity: severity(expensive_level),
                        message: format!("`{name}` is recomputed inside a loop"),
                        primary_range: call.range,
                        related: Vec::new(),
                        fixes: Vec::new(),
                        source: DiagnosticSource::StanLint,
                    });
                }
            }
        }
        if vector_level != LintLevel::Allow
            && snapshot.syntax.nodes().iter().any(|node| {
                node.kind == SyntaxNodeKind::SamplingStatement
                    && loop_node.range.start <= node.range.start
                    && node.range.end <= loop_node.range.end
            })
        {
            output.push(Diagnostic {
                code: crate::DiagnosticCode(VECTORIZATION_OPPORTUNITY.id),
                severity: severity(vector_level),
                message: "sampling statement in a loop may be vectorizable".to_owned(),
                primary_range: loop_node.range,
                related: Vec::new(),
                fixes: Vec::new(),
                source: DiagnosticSource::StanLint,
            });
        }
    }
}

fn context_at(snapshot: &AnalysisSnapshot, offset: u32) -> Option<CallContext> {
    snapshot
        .syntax
        .source_file()
        .program_blocks()
        .find_map(|block| {
            let range = block.range();
            if !(range.start <= offset && offset <= range.end) {
                return None;
            }
            match block.kind() {
                crate::ProgramBlockKind::TransformedData => Some(CallContext::TransformedData),
                crate::ProgramBlockKind::TransformedParameters => {
                    Some(CallContext::TransformedParameters)
                }
                crate::ProgramBlockKind::Model => Some(CallContext::Model),
                crate::ProgramBlockKind::GeneratedQuantities => {
                    Some(CallContext::GeneratedQuantities)
                }
                _ => None,
            }
        })
}

fn complete_call(snapshot: &AnalysisSnapshot, range: crate::TextRange) -> bool {
    snapshot
        .syntax
        .source_file()
        .call_expressions()
        .find(|call| call.range() == range)
        .is_some_and(|call| call.is_complete() && !call.is_declaration())
}

fn complete_sampling(snapshot: &AnalysisSnapshot, range: crate::TextRange) -> bool {
    snapshot
        .syntax
        .source_file()
        .sampling_statements()
        .find(|statement| statement.range() == range)
        .is_some_and(|statement| statement.is_complete())
}

fn legacy_fix(element: LegacyLanguageElement, range: crate::TextRange) -> Option<Fix> {
    (element == LegacyLanguageElement::ArrowAssignment).then(|| Fix {
        label: "replace `<-` with `=`".to_owned(),
        edits: vec![TextEdit {
            range,
            replacement: "=".to_owned(),
        }],
        applicability: Applicability::Always,
    })
}

fn severity(level: LintLevel) -> Severity {
    match level {
        LintLevel::Allow => unreachable!("allowed lints are not emitted"),
        LintLevel::Hint => Severity::Hint,
        LintLevel::Warn => Severity::Warning,
        LintLevel::Deny => Severity::Error,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Revision, analyze_revision};

    #[test]
    fn lint_levels_are_configurable() {
        let snapshot = analyze_revision("model { increment_log_prob(1); }", Revision::default());
        assert!(
            lint(&snapshot, &LintConfig::default())
                .iter()
                .any(|diagnostic| diagnostic.code.0 == DEPRECATED_LANGUAGE_ELEMENT.id)
        );
        let mut config = LintConfig::default();
        config.set(DEPRECATED_LANGUAGE_ELEMENT.id, LintLevel::Allow);
        assert!(
            !lint(&snapshot, &config)
                .iter()
                .any(|diagnostic| diagnostic.code.0 == DEPRECATED_LANGUAGE_ELEMENT.id)
        );
    }

    #[test]
    fn correctness_and_bayesian_lints_are_tiered() {
        let snapshot = analyze_revision(
            "parameters { real theta; } model { real x; x = missing; }",
            Revision::default(),
        );
        let diagnostics = lint(&snapshot, &LintConfig::default());
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code.0 == UNRESOLVED_IDENTIFIER.id)
        );
        assert!(
            !diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code.0 == PARAMETER_WITHOUT_PRIOR.id)
        );
        let mut config = LintConfig::default();
        config.set(PARAMETER_WITHOUT_PRIOR.id, LintLevel::Hint);
        assert!(
            lint(&snapshot, &config)
                .iter()
                .any(|diagnostic| diagnostic.code.0 == PARAMETER_WITHOUT_PRIOR.id)
        );
    }

    #[test]
    fn uncertain_default_lints_are_hints() {
        for descriptor in [
            UNRESOLVED_IDENTIFIER,
            UNUSED_DECLARATION,
            ILLEGAL_CALL_CONTEXT,
            ARGUMENT_TYPE,
            UNKNOWN_DISTRIBUTION,
            REPEATED_EXPENSIVE_OPERATION,
            VECTORIZATION_OPPORTUNITY,
        ] {
            assert_eq!(
                descriptor.default_level,
                LintLevel::Hint,
                "{}",
                descriptor.id
            );
            assert_eq!(descriptor.stability, LintStability::Experimental);
        }
        assert_eq!(ARGUMENT_COUNT.default_level, LintLevel::Deny);
        assert_eq!(PARAMETER_WITHOUT_PRIOR.default_level, LintLevel::Allow);
    }

    #[test]
    fn incomplete_calls_do_not_produce_semantic_call_diagnostics() {
        let snapshot = analyze_revision("model { normal(", Revision::default());
        let diagnostics = lint(&snapshot, &LintConfig::default());
        for code in [ARGUMENT_COUNT.id, ARGUMENT_TYPE.id, ILLEGAL_CALL_CONTEXT.id] {
            assert!(
                !diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.code.0 == code)
            );
        }
    }

    #[test]
    fn user_defined_distributions_are_not_unknown() {
        let snapshot = analyze_revision(
            "functions { real custom_lpdf(real y, real theta) { return normal_lpdf(y | theta, 1); } } data { real y; } parameters { real theta; } model { y ~ custom(theta); }",
            Revision::default(),
        );
        assert!(
            !lint(&snapshot, &LintConfig::default())
                .iter()
                .any(|diagnostic| diagnostic.code.0 == UNKNOWN_DISTRIBUTION.id)
        );
    }

    #[test]
    fn source_suppressions_apply_to_the_following_line() {
        let snapshot = analyze_revision(
            "model {\n// stanlint: allow correctness.unresolved-identifier\nmissing = 1;\n}",
            Revision::default(),
        );
        assert!(
            !lint(&snapshot, &LintConfig::default())
                .iter()
                .any(|diagnostic| diagnostic.code.0 == UNRESOLVED_IDENTIFIER.id)
        );
    }

    #[test]
    fn certain_argument_count_mismatches_are_reported() {
        let snapshot = analyze_revision("model { real x; x = exp(1, 2); }", Revision::default());
        assert!(
            lint(&snapshot, &LintConfig::default())
                .iter()
                .any(|diagnostic| diagnostic.code.0 == ARGUMENT_COUNT.id)
        );
    }

    #[test]
    fn certain_argument_type_mismatches_are_reported() {
        let snapshot = analyze_revision(
            "model { real x; x = bernoulli_lpmf(1.2, 0.5); }",
            Revision::default(),
        );
        assert!(
            lint(&snapshot, &LintConfig::default())
                .iter()
                .any(|diagnostic| diagnostic.code.0 == ARGUMENT_TYPE.id)
        );
    }

    #[test]
    fn unknown_distributions_and_loop_costs_are_reported() {
        let unknown = analyze_revision(
            "model { real y; y ~ not_a_distribution(1); }",
            Revision::default(),
        );
        assert!(
            lint(&unknown, &LintConfig::default())
                .iter()
                .any(|diagnostic| diagnostic.code.0 == UNKNOWN_DISTRIBUTION.id)
        );

        let looped = analyze_revision(
            "model { matrix[2,2] m; for (n in 1:2) { m = inverse(m); } }",
            Revision::default(),
        );
        assert!(
            lint(&looped, &LintConfig::default())
                .iter()
                .any(|diagnostic| diagnostic.code.0 == REPEATED_EXPENSIVE_OPERATION.id)
        );
    }
}
