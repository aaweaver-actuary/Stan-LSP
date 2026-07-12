use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::time::Instant;

use serde::Serialize;
use stan_language::Revision;

const VERSION: &str = "2.39.0";

const SPECIAL_SIGNATURES: &[(&str, &str, &str)] = &[
    (
        "reduce_sum",
        "reduce_sum(F f, array[] T x, int grainsize, T1 shared...) => real",
        "real",
    ),
    (
        "reduce_sum_static",
        "reduce_sum_static(F f, array[] T x, int grainsize, T1 shared...) => real",
        "real",
    ),
    (
        "ode_adams",
        "ode_adams(F f, vector y0, real t0, array[] real ts, T1 args...) => array[] vector",
        "array[] vector",
    ),
    (
        "ode_adams_tol",
        "ode_adams_tol(F f, vector y0, real t0, array[] real ts, data real rtol, data real atol, data int max_steps, T1 args...) => array[] vector",
        "array[] vector",
    ),
    (
        "ode_bdf",
        "ode_bdf(F f, vector y0, real t0, array[] real ts, T1 args...) => array[] vector",
        "array[] vector",
    ),
    (
        "ode_bdf_tol",
        "ode_bdf_tol(F f, vector y0, real t0, array[] real ts, data real rtol, data real atol, data int max_steps, T1 args...) => array[] vector",
        "array[] vector",
    ),
    (
        "ode_ckrk",
        "ode_ckrk(F f, vector y0, real t0, array[] real ts, T1 args...) => array[] vector",
        "array[] vector",
    ),
    (
        "ode_ckrk_tol",
        "ode_ckrk_tol(F f, vector y0, real t0, array[] real ts, data real rtol, data real atol, data int max_steps, T1 args...) => array[] vector",
        "array[] vector",
    ),
    (
        "ode_rk45",
        "ode_rk45(F f, vector y0, real t0, array[] real ts, T1 args...) => array[] vector",
        "array[] vector",
    ),
    (
        "ode_rk45_tol",
        "ode_rk45_tol(F f, vector y0, real t0, array[] real ts, data real rtol, data real atol, data int max_steps, T1 args...) => array[] vector",
        "array[] vector",
    ),
    (
        "dae",
        "dae(F residual, vector yy0, vector yp0, real t0, array[] real ts, T1 args...) => array[] vector",
        "array[] vector",
    ),
    (
        "dae_tol",
        "dae_tol(F residual, vector yy0, vector yp0, real t0, array[] real ts, data real rtol, data real atol, data int max_steps, T1 args...) => array[] vector",
        "array[] vector",
    ),
    (
        "solve_newton",
        "solve_newton(F f, vector y_guess, T1 args...) => vector",
        "vector",
    ),
    (
        "solve_newton_tol",
        "solve_newton_tol(F f, vector y_guess, data real scaling_step, data real f_tol, data int max_steps, T1 args...) => vector",
        "vector",
    ),
    (
        "solve_powell",
        "solve_powell(F f, vector y_guess, T1 args...) => vector",
        "vector",
    ),
    (
        "solve_powell_tol",
        "solve_powell_tol(F f, vector y_guess, data real rel_tol, data real f_tol, data int max_steps, T1 args...) => vector",
        "vector",
    ),
    (
        "laplace_marginal",
        "laplace_marginal(F log_likelihood, F covariance, tuple data, tuple hyperparameters...) => real",
        "real",
    ),
    (
        "laplace_marginal_tol",
        "laplace_marginal_tol(F log_likelihood, F covariance, tuple data, data real tolerance, tuple hyperparameters...) => real",
        "real",
    ),
    (
        "laplace_latent_rng",
        "laplace_latent_rng(F log_likelihood, F covariance, tuple data, tuple hyperparameters...) => vector",
        "vector",
    ),
    (
        "laplace_latent_tol_rng",
        "laplace_latent_tol_rng(F log_likelihood, F covariance, tuple data, data real tolerance, tuple hyperparameters...) => vector",
        "vector",
    ),
    (
        "laplace_marginal_poisson_log_lpmf",
        "laplace_marginal_poisson_log_lpmf(array[] int y | array[] int y_index, vector m, data int hessian_block_size, F covariance, tuple covariance_arguments) => real",
        "real",
    ),
    (
        "laplace_marginal_poisson_log_lupmf",
        "laplace_marginal_poisson_log_lupmf(array[] int y | array[] int y_index, vector m, data int hessian_block_size, F covariance, tuple covariance_arguments) => real",
        "real",
    ),
    (
        "laplace_marginal_tol_poisson_log_lpmf",
        "laplace_marginal_tol_poisson_log_lpmf(array[] int y | array[] int y_index, vector m, data int hessian_block_size, F covariance, tuple covariance_arguments, tuple tolerances) => real",
        "real",
    ),
    (
        "laplace_marginal_tol_poisson_log_lupmf",
        "laplace_marginal_tol_poisson_log_lupmf(array[] int y | array[] int y_index, vector m, data int hessian_block_size, F covariance, tuple covariance_arguments, tuple tolerances) => real",
        "real",
    ),
    (
        "laplace_latent_poisson_log_rng",
        "laplace_latent_poisson_log_rng(array[] int y, array[] int y_index, vector m, data int hessian_block_size, F covariance, tuple covariance_arguments) => vector",
        "vector",
    ),
    (
        "laplace_latent_tol_poisson_log_rng",
        "laplace_latent_tol_poisson_log_rng(array[] int y, array[] int y_index, vector m, data int hessian_block_size, F covariance, tuple covariance_arguments, tuple tolerances) => vector",
        "vector",
    ),
    (
        "laplace_marginal_neg_binomial_2_log_lpmf",
        "laplace_marginal_neg_binomial_2_log_lpmf(array[] int y | array[] int y_index, real eta, vector m, data int hessian_block_size, F covariance, tuple covariance_arguments) => real",
        "real",
    ),
    (
        "laplace_marginal_neg_binomial_2_log_lupmf",
        "laplace_marginal_neg_binomial_2_log_lupmf(array[] int y | array[] int y_index, real eta, vector m, data int hessian_block_size, F covariance, tuple covariance_arguments) => real",
        "real",
    ),
    (
        "laplace_marginal_tol_neg_binomial_2_log_lpmf",
        "laplace_marginal_tol_neg_binomial_2_log_lpmf(array[] int y | array[] int y_index, real eta, vector m, data int hessian_block_size, F covariance, tuple covariance_arguments, tuple tolerances) => real",
        "real",
    ),
    (
        "laplace_marginal_tol_neg_binomial_2_log_lupmf",
        "laplace_marginal_tol_neg_binomial_2_log_lupmf(array[] int y | array[] int y_index, real eta, vector m, data int hessian_block_size, F covariance, tuple covariance_arguments, tuple tolerances) => real",
        "real",
    ),
    (
        "laplace_latent_neg_binomial_2_log_rng",
        "laplace_latent_neg_binomial_2_log_rng(array[] int y, array[] int y_index, real eta, vector m, data int hessian_block_size, F covariance, tuple covariance_arguments) => vector",
        "vector",
    ),
    (
        "laplace_latent_tol_neg_binomial_2_log_rng",
        "laplace_latent_tol_neg_binomial_2_log_rng(array[] int y, array[] int y_index, real eta, vector m, data int hessian_block_size, F covariance, tuple covariance_arguments, tuple tolerances) => vector",
        "vector",
    ),
    (
        "laplace_marginal_bernoulli_logit_lpmf",
        "laplace_marginal_bernoulli_logit_lpmf(array[] int y | array[] int y_index, vector m, data int hessian_block_size, F covariance, tuple covariance_arguments) => real",
        "real",
    ),
    (
        "laplace_marginal_bernoulli_logit_lupmf",
        "laplace_marginal_bernoulli_logit_lupmf(array[] int y | array[] int y_index, vector m, data int hessian_block_size, F covariance, tuple covariance_arguments) => real",
        "real",
    ),
    (
        "laplace_marginal_tol_bernoulli_logit_lpmf",
        "laplace_marginal_tol_bernoulli_logit_lpmf(array[] int y | array[] int y_index, vector m, data int hessian_block_size, F covariance, tuple covariance_arguments, tuple tolerances) => real",
        "real",
    ),
    (
        "laplace_marginal_tol_bernoulli_logit_lupmf",
        "laplace_marginal_tol_bernoulli_logit_lupmf(array[] int y | array[] int y_index, vector m, data int hessian_block_size, F covariance, tuple covariance_arguments, tuple tolerances) => real",
        "real",
    ),
    (
        "laplace_latent_bernoulli_logit_rng",
        "laplace_latent_bernoulli_logit_rng(array[] int y, array[] int y_index, vector m, data int hessian_block_size, F covariance, tuple covariance_arguments) => vector",
        "vector",
    ),
    (
        "laplace_latent_tol_bernoulli_logit_rng",
        "laplace_latent_tol_bernoulli_logit_rng(array[] int y, array[] int y_index, vector m, data int hessian_block_size, F covariance, tuple covariance_arguments, tuple tolerances) => vector",
        "vector",
    ),
];

const REMOVED_FUNCTIONS: &[&str] = &[
    "binomial_coefficient_log",
    "cov_exp_quad",
    "fabs",
    "get_lp",
    "multiply_log",
];

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let mut args = env::args().skip(1);
    let command = args.next();
    if command.as_deref() == Some("validate") {
        return validate_catalog();
    }
    if command.as_deref() == Some("corpus") {
        return corpus_command(args.collect());
    }
    if command.as_deref() != Some("refresh-stan") {
        return Err(
            "usage: cargo run -p xtask -- validate | refresh-stan --stanc <path> [--check] | corpus <fetch|check> [--external] [--stanc <path>] [--json <path>]"
                .to_owned(),
        );
    }

    let mut stanc = None;
    let mut check = false;
    while let Some(argument) = args.next() {
        match argument.as_str() {
            "--stanc" => stanc = args.next().map(PathBuf::from),
            "--check" => check = true,
            other => return Err(format!("unknown argument {other:?}")),
        }
    }
    let stanc = stanc.ok_or_else(|| "--stanc <path> is required".to_owned())?;
    refresh(&stanc, check)
}

#[derive(Debug, Default, Serialize)]
struct CorpusReport {
    models_analyzed: usize,
    parser_or_analysis_panics: usize,
    formatter_panics: usize,
    semantic_invariant_failures: usize,
    invalid_ranges: usize,
    formatter_errors: usize,
    formatter_non_idempotence: usize,
    stanc_valid_models: usize,
    formatted_stanc_failures: usize,
    default_local_errors_on_stanc_valid_models: usize,
    elapsed_milliseconds: u128,
    failures: Vec<String>,
}

fn corpus_command(arguments: Vec<String>) -> Result<(), String> {
    let Some(action) = arguments.first().map(String::as_str) else {
        return Err("corpus requires `fetch` or `check`".to_owned());
    };
    let root = workspace_root()?;
    if action == "fetch" {
        return fetch_corpus(&root);
    }
    if action != "check" {
        return Err(format!("unknown corpus action {action:?}"));
    }
    let mut external = false;
    let mut stanc = None;
    let mut json_path = None;
    let mut cursor = 1usize;
    while cursor < arguments.len() {
        match arguments[cursor].as_str() {
            "--external" => external = true,
            "--stanc" => {
                cursor += 1;
                stanc = arguments.get(cursor).map(PathBuf::from);
                if stanc.is_none() {
                    return Err("--stanc requires a path".to_owned());
                }
            }
            "--json" => {
                cursor += 1;
                json_path = arguments.get(cursor).map(PathBuf::from);
                if json_path.is_none() {
                    return Err("--json requires a path".to_owned());
                }
            }
            other => return Err(format!("unknown corpus argument {other:?}")),
        }
        cursor += 1;
    }
    let report = check_corpus(&root, external, stanc.as_deref())?;
    let rendered = serde_json::to_string_pretty(&report)
        .map_err(|error| format!("failed to serialize corpus report: {error}"))?;
    if let Some(path) = json_path {
        fs::write(&path, &rendered)
            .map_err(|error| format!("failed to write {}: {error}", path.display()))?;
    }
    println!("{rendered}");
    if report.parser_or_analysis_panics > 0
        || report.formatter_panics > 0
        || report.semantic_invariant_failures > 0
        || report.invalid_ranges > 0
        || report.formatter_non_idempotence > 0
        || report.formatted_stanc_failures > 0
        || report.default_local_errors_on_stanc_valid_models > 0
    {
        return Err("corpus beta gate failed".to_owned());
    }
    Ok(())
}

fn workspace_root() -> Result<PathBuf, String> {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| "xtask has no workspace parent".to_owned())
}

fn fetch_corpus(root: &Path) -> Result<(), String> {
    let manifest_path = root.join("corpus/manifest.toml");
    let manifest = fs::read_to_string(&manifest_path)
        .map_err(|error| format!("failed to read {}: {error}", manifest_path.display()))?;
    let url = manifest_value(&manifest, "archive_url")?;
    let expected = manifest_value(&manifest, "sha256")?;
    let cache = root.join("target/corpus");
    fs::create_dir_all(&cache)
        .map_err(|error| format!("failed to create {}: {error}", cache.display()))?;
    let archive = cache.join("external.tar.gz");
    run_status(
        Command::new("curl")
            .args([
                "--fail",
                "--location",
                "--silent",
                "--show-error",
                "--output",
            ])
            .arg(&archive)
            .arg(url),
    )?;
    let actual = checksum(&archive)?;
    if actual != expected {
        return Err(format!(
            "external corpus checksum mismatch: expected {expected}, got {actual}"
        ));
    }
    let external = cache.join("external");
    if external.exists() {
        fs::remove_dir_all(&external)
            .map_err(|error| format!("failed to clear {}: {error}", external.display()))?;
    }
    fs::create_dir_all(&external)
        .map_err(|error| format!("failed to create {}: {error}", external.display()))?;
    run_status(
        Command::new("tar")
            .args(["-xzf"])
            .arg(&archive)
            .arg("-C")
            .arg(&external),
    )?;
    println!("fetched pinned external corpus into {}", external.display());
    Ok(())
}

fn manifest_value<'a>(manifest: &'a str, key: &str) -> Result<&'a str, String> {
    manifest
        .lines()
        .find_map(|line| {
            let (candidate, value) = line.split_once('=')?;
            (candidate.trim() == key).then(|| value.trim().trim_matches('"'))
        })
        .ok_or_else(|| format!("corpus manifest has no {key:?}"))
}

fn checksum(path: &Path) -> Result<String, String> {
    let output = Command::new("sha256sum")
        .arg(path)
        .output()
        .or_else(|_| {
            Command::new("shasum")
                .args(["-a", "256"])
                .arg(path)
                .output()
        })
        .map_err(|error| format!("failed to run SHA-256 tool: {error}"))?;
    if !output.status.success() {
        return Err("SHA-256 tool failed".to_owned());
    }
    String::from_utf8_lossy(&output.stdout)
        .split_whitespace()
        .next()
        .map(str::to_owned)
        .ok_or_else(|| "SHA-256 tool returned no digest".to_owned())
}

fn check_corpus(root: &Path, external: bool, stanc: Option<&Path>) -> Result<CorpusReport, String> {
    let started = Instant::now();
    let mut paths = stan_files(&root.join("corpus/smoke"))?;
    if external {
        let external_root = root.join("target/corpus/external");
        if !external_root.exists() {
            return Err("external corpus is missing; run `xtask corpus fetch`".to_owned());
        }
        paths.extend(stan_files(&external_root)?);
    }
    paths.sort();
    paths.dedup();
    let mut report = CorpusReport::default();
    for path in paths {
        let source = fs::read_to_string(&path)
            .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
        report.models_analyzed += 1;
        let analysis = match std::panic::catch_unwind(|| {
            stan_language::analyze_revision(&source, Revision::default())
        }) {
            Ok(analysis) => analysis,
            Err(_) => {
                report.parser_or_analysis_panics += 1;
                report
                    .failures
                    .push(format!("analysis panic: {}", path.display()));
                continue;
            }
        };
        if analysis.syntax.reconstruct() != source
            || analysis.tokens.iter().any(|token| {
                token.range.start > token.range.end || token.range.end as usize > source.len()
            })
            || analysis.diagnostics.iter().any(|diagnostic| {
                diagnostic.primary_range.start > diagnostic.primary_range.end
                    || diagnostic.primary_range.end as usize > source.len()
            })
        {
            report.invalid_ranges += 1;
            report.failures.push(format!(
                "invalid range or reconstruction: {}",
                path.display()
            ));
        }
        #[cfg(debug_assertions)]
        if let Err(error) = analysis.semantics.validate() {
            report.semantic_invariant_failures += 1;
            report
                .failures
                .push(format!("semantic invariant {error:?}: {}", path.display()));
        }
        let formatted = match std::panic::catch_unwind(|| {
            stan_language::format(&analysis, &stan_language::FormatterConfig::default())
        }) {
            Ok(Ok(formatted)) => Some(formatted),
            Ok(Err(_)) => {
                report.formatter_errors += 1;
                None
            }
            Err(_) => {
                report.formatter_panics += 1;
                report
                    .failures
                    .push(format!("formatter panic: {}", path.display()));
                None
            }
        };
        if let Some(formatted) = &formatted {
            match stan_language::format(
                &stan_language::analyze_revision(formatted, Revision::default()),
                &stan_language::FormatterConfig::default(),
            ) {
                Ok(second) if second == *formatted => {}
                _ => {
                    report.formatter_non_idempotence += 1;
                    report
                        .failures
                        .push(format!("formatter non-idempotence: {}", path.display()));
                }
            }
        }
        if let Some(stanc) = stanc {
            if compiler_accepts(stanc, &path) {
                report.stanc_valid_models += 1;
                let local_errors = analysis
                    .diagnostics
                    .iter()
                    .cloned()
                    .chain(stan_language::lint(
                        &analysis,
                        &stan_language::LintConfig::default(),
                    ))
                    .filter(|diagnostic| diagnostic.severity == stan_language::Severity::Error)
                    .count();
                report.default_local_errors_on_stanc_valid_models += local_errors;
                if let Some(formatted) = formatted {
                    let temp = root.join("target/corpus/formatted.stan");
                    fs::write(&temp, formatted)
                        .map_err(|error| format!("failed to write {}: {error}", temp.display()))?;
                    if !compiler_accepts(stanc, &temp) {
                        report.formatted_stanc_failures += 1;
                        report
                            .failures
                            .push(format!("formatted source rejected: {}", path.display()));
                    }
                }
            }
        }
    }
    report.elapsed_milliseconds = started.elapsed().as_millis();
    Ok(report)
}

fn stan_files(root: &Path) -> Result<Vec<PathBuf>, String> {
    let mut files = Vec::new();
    if !root.exists() {
        return Ok(files);
    }
    let mut pending = vec![root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(&directory)
            .map_err(|error| format!("failed to read {}: {error}", directory.display()))?
        {
            let path = entry.map_err(|error| error.to_string())?.path();
            if path.is_dir() {
                pending.push(path);
            } else if path.extension().and_then(|extension| extension.to_str()) == Some("stan") {
                files.push(path);
            }
        }
    }
    Ok(files)
}

fn compiler_accepts(stanc: &Path, source: &Path) -> bool {
    Command::new(stanc)
        .arg(source)
        .output()
        .is_ok_and(|output| output.status.success())
}

fn run_status(command: &mut Command) -> Result<(), String> {
    let status = command
        .status()
        .map_err(|error| format!("failed to run {command:?}: {error}"))?;
    status
        .success()
        .then_some(())
        .ok_or_else(|| format!("{command:?} failed with {status}"))
}

fn validate_catalog() -> Result<(), String> {
    let catalog = stan_language::FunctionCatalog::global();
    for function in stan_language::StanFunction::ALL {
        let metadata = catalog.metadata(*function);
        if metadata.categories.is_empty() {
            return Err(format!("{function} has no category"));
        }
        if metadata
            .lifecycle
            .is_available_in(stan_language::STAN_VERSION)
            && catalog.signatures(*function).is_empty()
        {
            return Err(format!("{function} has no signature"));
        }
    }
    println!(
        "validated {} Stan {} built-in functions",
        stan_language::StanFunction::ALL.len(),
        stan_language::STAN_VERSION
    );
    Ok(())
}

fn refresh(stanc: &Path, check: bool) -> Result<(), String> {
    let version = command_output(Command::new(stanc).arg("--version"))?;
    if !version.contains(&format!("stanc3 v{VERSION}")) {
        return Err(format!("expected stanc3 v{VERSION}, got {version:?}"));
    }

    let dump = command_output(Command::new(stanc).arg("--dump-stan-math-signatures"))?;
    let mut signatures = dump
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_owned)
        .collect::<BTreeSet<_>>();
    synthesize_unnormalized_probability_functions(&mut signatures)?;

    let mut by_name = BTreeMap::<String, Vec<String>>::new();
    for signature in &signatures {
        let name = signature
            .split_once('(')
            .map(|(name, _)| name)
            .ok_or_else(|| format!("malformed signature {signature:?}"))?;
        by_name
            .entry(name.to_owned())
            .or_default()
            .push(signature.clone());
    }
    for (name, _, _) in SPECIAL_SIGNATURES {
        by_name.entry((*name).to_owned()).or_default();
    }
    for name in REMOVED_FUNCTIONS {
        by_name.entry((*name).to_owned()).or_default();
    }

    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or_else(|| "xtask has no workspace parent".to_owned())?;
    let signature_path = root.join("catalog/stan-2.39/signatures.txt");
    let special_path = root.join("catalog/stan-2.39/special-signatures.tsv");
    let availability_path = root.join("catalog/stan-2.39/availability.tsv");
    let function_path = root.join("crates/stan-language/src/generated_functions.rs");
    let distribution_path = root.join("crates/stan-language/src/generated_distributions.rs");
    let availability = read_availability(&availability_path)?;

    let signature_content = render_signatures(&signatures);
    let special_content = render_special_signatures();
    let function_content = render_functions(&by_name, &availability)?;
    let distribution_content = render_distributions(by_name.keys())?;

    update(&signature_path, &signature_content, check)?;
    update(&special_path, &special_content, check)?;
    update(&function_path, &function_content, check)?;
    update(&distribution_path, &distribution_content, check)?;
    Ok(())
}

fn synthesize_unnormalized_probability_functions(
    signatures: &mut BTreeSet<String>,
) -> Result<(), String> {
    let originals = signatures.iter().cloned().collect::<Vec<_>>();
    for signature in originals {
        let open = signature
            .find('(')
            .ok_or_else(|| format!("malformed signature {signature:?}"))?;
        let name = &signature[..open];
        let replacement = if let Some(base) = name.strip_suffix("_lpdf") {
            Some(format!("{base}_lupdf{}", &signature[open..]))
        } else {
            name.strip_suffix("_lpmf")
                .map(|base| format!("{base}_lupmf{}", &signature[open..]))
        };
        if let Some(replacement) = replacement {
            signatures.insert(replacement);
        }
    }
    Ok(())
}

fn render_signatures(signatures: &BTreeSet<String>) -> String {
    let mut output = format!(
        "# Generated from stanc3 v{VERSION} --dump-stan-math-signatures.\n# Unnormalized probability overloads are derived from their normalized counterparts.\n"
    );
    for signature in signatures {
        writeln!(output, "{signature}").unwrap();
    }
    output
}

fn render_special_signatures() -> String {
    let mut output = "# name\tdisplay signature\treturn type\n".to_owned();
    for (name, display, return_type) in SPECIAL_SIGNATURES {
        writeln!(output, "{name}\t{display}\t{return_type}").unwrap();
    }
    output
}

fn render_functions(
    by_name: &BTreeMap<String, Vec<String>>,
    availability: &BTreeMap<String, (u16, u16)>,
) -> Result<String, String> {
    let mut output = "// @generated by `cargo run -p xtask -- refresh-stan`; do not edit by hand.\n\n#[rustfmt::skip]\n#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Token)]\npub enum StanFunction {\n".to_owned();
    let mut variants = BTreeMap::new();
    for name in by_name.keys() {
        let variant = rust_variant(name);
        if let Some(other) = variants.insert(variant.clone(), name.clone()) {
            return Err(format!(
                "function names {other:?} and {name:?} map to variant {variant}"
            ));
        }
        writeln!(output, "    #[token({name:?})]\n    {variant},").unwrap();
    }
    output.push_str("}\n");
    output.push_str("\n#[rustfmt::skip]\nimpl StanFunction {\n    pub const fn declared_categories(&self) -> &'static [FunctionCategory] {\n        match self {\n");
    for (name, signatures) in by_name {
        let categories = categories_for(name, signatures);
        let category_values = categories
            .iter()
            .map(|category| format!("FunctionCategory::{category}"))
            .collect::<Vec<_>>()
            .join(", ");
        writeln!(
            output,
            "            Self::{} => &[{}],",
            rust_variant(name),
            category_values,
        )
        .unwrap();
    }
    output.push_str("        }\n    }\n\n    pub const fn declared_lifecycle(&self) -> Lifecycle {\n        match self {\n");
    for name in by_name.keys() {
        writeln!(
            output,
            "            Self::{} => {},",
            rust_variant(name),
            lifecycle_expression(name, availability),
        )
        .unwrap();
    }
    output.push_str("        }\n    }\n\n    pub const fn declared_call_contexts(&self) -> CallContextSet {\n        match self {\n");
    for name in by_name.keys() {
        writeln!(
            output,
            "            Self::{} => {},",
            rust_variant(name),
            call_context_expression(name),
        )
        .unwrap();
    }
    output.push_str("        }\n    }\n}\n");
    Ok(output)
}

fn call_context_expression(name: &str) -> &'static str {
    if name.ends_with("_rng") {
        "CallContextSet::TRANSFORMED_DATA.union(CallContextSet::GENERATED_QUANTITIES).union(CallContextSet::RNG_FUNCTION)"
    } else if name.ends_with("_jacobian") {
        "CallContextSet::TRANSFORMED_PARAMETERS.union(CallContextSet::JACOBIAN_FUNCTION)"
    } else if name.ends_with("_lupdf") || name.ends_with("_lupmf") {
        "CallContextSet::MODEL.union(CallContextSet::LOG_PROBABILITY_FUNCTION)"
    } else {
        "CallContextSet::ANY"
    }
}

fn categories_for(name: &str, signatures: &[String]) -> Vec<&'static str> {
    let mut categories = BTreeSet::new();
    if name.starts_with("hmm_") {
        categories.insert("HiddenMarkov");
    }
    if name.starts_with("laplace_") {
        categories.insert("EmbeddedLaplace");
    }
    if name.starts_with("csr_") || name.contains("sparse") {
        categories.insert("SparseMatrix");
    }
    if name.ends_with("_constrain") || name.ends_with("_unconstrain") || name.ends_with("_jacobian")
    {
        categories.insert("Transform");
    }
    if [
        "_lpdf", "_lpmf", "_lupdf", "_lupmf", "_cdf", "_lcdf", "_lccdf", "_rng",
    ]
    .iter()
    .any(|suffix| name.ends_with(suffix))
    {
        categories.insert("Probability");
    }
    if is_higher_order(name)
        || SPECIAL_SIGNATURES
            .iter()
            .any(|(special, _, _)| special == &name)
    {
        categories.insert("HigherOrder");
    }
    let joined = signatures.join("\n");
    if joined.contains("array[") {
        categories.insert("Array");
    }
    if joined.contains("complex_matrix")
        || joined.contains("complex_vector")
        || joined.contains("complex_row_vector")
    {
        categories.insert("ComplexMatrix");
    } else if joined.contains("complex") {
        categories.insert("ComplexMath");
    }
    if joined.contains("matrix") || joined.contains("vector") || joined.contains("row_vector") {
        categories.insert("Matrix");
    }
    if joined.contains("tuple(") || joined.matches("=>").count() > signatures.len() {
        categories.insert("Mixed");
    }
    if joined.contains("int") || joined.contains("real") {
        categories.insert("ScalarMath");
    }
    if categories.is_empty() {
        categories.insert("Utility");
    }
    categories.into_iter().collect()
}

fn is_higher_order(name: &str) -> bool {
    name.starts_with("ode_")
        || name.starts_with("dae")
        || name.starts_with("solve_")
        || matches!(
            name,
            "integrate_1d" | "integrate_ode" | "reduce_sum" | "reduce_sum_static" | "map_rect"
        )
}

fn lifecycle_expression(name: &str, availability: &BTreeMap<String, (u16, u16)>) -> String {
    let introduced = availability
        .get(name)
        .map(|(major, minor)| format!("Some(StanVersion::new({major}, {minor}, 0))"))
        .unwrap_or_else(|| "None".to_owned());
    match name {
        "algebra_solver" | "algebra_solver_newton" => format!(
            "Lifecycle::Deprecated {{ introduced: {introduced}, deprecated: StanVersion::new(2, 28, 0), replacement: Some(\"solve_newton or solve_powell\") }}"
        ),
        "integrate_ode_adams" | "integrate_ode_bdf" | "integrate_ode_rk45" => format!(
            "Lifecycle::Deprecated {{ introduced: {introduced}, deprecated: StanVersion::new(2, 24, 0), replacement: Some(\"ode_adams, ode_bdf, or ode_rk45\") }}"
        ),
        "multiply_log" | "binomial_coefficient_log" | "get_lp" | "fabs" | "cov_exp_quad" => {
            "Lifecycle::Removed { removed: StanVersion::new(2, 39, 0), replacement: None }"
                .to_owned()
        }
        _ if name.starts_with("laplace_")
            || name.starts_with("yule_simon")
            || name == "generate_laplace_options" =>
        {
            "Lifecycle::Active { introduced: Some(StanVersion::new(2, 39, 0)) }".to_owned()
        }
        _ => format!("Lifecycle::Active {{ introduced: {introduced} }}"),
    }
}

fn read_availability(path: &Path) -> Result<BTreeMap<String, (u16, u16)>, String> {
    let input = fs::read_to_string(path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    let mut availability = BTreeMap::new();
    for (line_number, raw_line) in input.lines().enumerate() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (name, version) = line.split_once('\t').ok_or_else(|| {
            format!(
                "{}:{} has no tab separator",
                path.display(),
                line_number + 1
            )
        })?;
        let (major, minor) = version.split_once('.').ok_or_else(|| {
            format!(
                "{}:{} has invalid version {version:?}",
                path.display(),
                line_number + 1
            )
        })?;
        let value = (
            major
                .parse::<u16>()
                .map_err(|error| format!("invalid major version {major:?}: {error}"))?,
            minor
                .parse::<u16>()
                .map_err(|error| format!("invalid minor version {minor:?}: {error}"))?,
        );
        if availability.insert(name.to_owned(), value).is_some() {
            return Err(format!(
                "{}:{} duplicates {name:?}",
                path.display(),
                line_number + 1
            ));
        }
    }
    Ok(availability)
}

fn render_distributions<'a>(names: impl Iterator<Item = &'a String>) -> Result<String, String> {
    let mut distributions = BTreeMap::<String, &'static str>::new();
    for name in names {
        let (base, kind) = if let Some(base) = name.strip_suffix("_lpmf") {
            (base, "Discrete")
        } else if let Some(base) = name.strip_suffix("_lpdf") {
            (base, "Continuous")
        } else {
            continue;
        };
        distributions.insert(base.to_owned(), kind);
    }

    let mut output = "// @generated by `cargo run -p xtask -- refresh-stan`; do not edit by hand.\n\n#[rustfmt::skip]\n#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Token)]\npub enum Distribution {\n".to_owned();
    let mut variants = BTreeMap::new();
    for name in distributions.keys() {
        let variant = rust_variant(name);
        if let Some(other) = variants.insert(variant.clone(), name.clone()) {
            return Err(format!(
                "distribution names {other:?} and {name:?} map to variant {variant}"
            ));
        }
        writeln!(output, "    #[token({name:?})]\n    {variant},").unwrap();
    }
    output.push_str("}\n\n#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]\npub enum DistributionKind {\n    Discrete,\n    Continuous,\n}\n\n#[rustfmt::skip]\nimpl Distribution {\n    pub const fn kind(&self) -> DistributionKind {\n        match self {\n");
    for (name, kind) in &distributions {
        writeln!(
            output,
            "            Self::{} => DistributionKind::{kind},",
            rust_variant(name)
        )
        .unwrap();
    }
    output.push_str("        }\n    }\n}\n");
    Ok(output)
}

fn rust_variant(name: &str) -> String {
    let mut output = String::new();
    let mut uppercase = true;
    for character in name.chars() {
        if character == '_' || !character.is_ascii_alphanumeric() {
            uppercase = true;
        } else if uppercase {
            output.extend(character.to_uppercase());
            uppercase = false;
        } else {
            output.push(character);
        }
    }
    if output
        .chars()
        .next()
        .is_some_and(|character| character.is_ascii_digit())
    {
        output.insert_str(0, "Function");
    }
    output
}

fn command_output(command: &mut Command) -> Result<String, String> {
    let output = command
        .output()
        .map_err(|error| format!("failed to run {command:?}: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "{command:?} failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    String::from_utf8(output.stdout)
        .map_err(|error| format!("command output was not UTF-8: {error}"))
}

fn update(path: &Path, content: &str, check: bool) -> Result<(), String> {
    let current = fs::read_to_string(path).unwrap_or_default();
    if current == content {
        return Ok(());
    }
    if check {
        return Err(format!("{} is out of date", path.display()));
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    }
    fs::write(path, content).map_err(|error| format!("failed to write {}: {error}", path.display()))
}
