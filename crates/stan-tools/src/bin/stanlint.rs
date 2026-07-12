use std::{
    env, fs,
    io::{self, Read},
    path::PathBuf,
    process::ExitCode,
};

use serde_json::json;
use stan_language::{LintConfig, Revision, Severity, analyze_revision, lint};

fn main() -> ExitCode {
    match run() {
        Ok(has_findings) if has_findings => ExitCode::from(1),
        Ok(_) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("stanlint: {error}");
            ExitCode::from(2)
        }
    }
}

fn run() -> Result<bool, String> {
    let config = fs::read_to_string("stanlint.toml")
        .ok()
        .map(|input| LintConfig::from_toml(&input))
        .transpose()?
        .unwrap_or_default();
    let mut json_output = false;
    let mut files = Vec::new();
    for argument in env::args().skip(1) {
        match argument.as_str() {
            "--json" => json_output = true,
            "-h" | "--help" => {
                println!("Usage: stanlint [--json] [FILES...]");
                return Ok(false);
            }
            value if value.starts_with('-') => return Err(format!("unknown option {value:?}")),
            value => files.push(PathBuf::from(value)),
        }
    }

    let inputs = if files.is_empty() {
        let mut source = String::new();
        io::stdin()
            .read_to_string(&mut source)
            .map_err(|error| error.to_string())?;
        vec![("stdin".to_owned(), source)]
    } else {
        files
            .into_iter()
            .map(|path| {
                fs::read_to_string(&path)
                    .map(|source| (path.display().to_string(), source))
                    .map_err(|error| format!("{}: {error}", path.display()))
            })
            .collect::<Result<Vec<_>, _>>()?
    };

    let mut findings = Vec::new();
    for (path, source) in inputs {
        let snapshot = analyze_revision(&source, Revision::default());
        for diagnostic in snapshot
            .diagnostics
            .iter()
            .cloned()
            .chain(lint(&snapshot, &config))
        {
            if json_output {
                findings.push(json!({
                    "path": path,
                    "code": diagnostic.code.0,
                    "severity": severity_name(diagnostic.severity),
                    "message": diagnostic.message,
                    "range": {"start": diagnostic.primary_range.start, "end": diagnostic.primary_range.end}
                }));
            } else {
                eprintln!(
                    "{path}:{}-{}: {}: {}",
                    diagnostic.primary_range.start,
                    diagnostic.primary_range.end,
                    diagnostic.code.0,
                    diagnostic.message
                );
                findings.push(serde_json::Value::Null);
            }
        }
    }
    if json_output {
        println!("{}", serde_json::to_string_pretty(&findings).unwrap());
    }
    Ok(!findings.is_empty())
}

fn severity_name(severity: Severity) -> &'static str {
    match severity {
        Severity::Error => "error",
        Severity::Warning => "warning",
        Severity::Information => "information",
        Severity::Hint => "hint",
    }
}
