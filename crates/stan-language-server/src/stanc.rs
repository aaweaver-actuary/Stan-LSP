//! Asynchronous integration with the authoritative stanc3 compiler.

#![allow(
    missing_docs,
    reason = "process adapter operations and failures are documented in docs/stanc-integration.md"
)]

use std::{
    env,
    io::Write,
    path::{Path, PathBuf},
    process::Stdio,
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};

use stan_language::{Diagnostic, DiagnosticCode, DiagnosticSource, Severity, TextRange};
use tokio::{process::Command, time::timeout};

#[derive(Debug, Clone)]
pub struct StancRunner {
    executable: PathBuf,
    timeout: Duration,
}

impl StancRunner {
    pub fn discover() -> Option<Self> {
        let executable = env::var_os("STANC")
            .map(PathBuf::from)
            .filter(|path| path.is_file())
            .or_else(find_on_path)
            .or_else(find_cmdstan)?;
        Some(Self::with_path(executable))
    }

    pub fn with_path(executable: impl Into<PathBuf>) -> Self {
        Self {
            executable: executable.into(),
            timeout: Duration::from_secs(30),
        }
    }

    pub async fn version(&self) -> Result<String, StancError> {
        let output = self.command().arg("--version").output();
        let output = timeout(self.timeout, output)
            .await
            .map_err(|_| StancError::Timeout)?
            .map_err(StancError::Io)?;
        if !output.status.success() {
            return Err(StancError::Failed(
                String::from_utf8_lossy(&output.stderr).into_owned(),
            ));
        }
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
    }

    pub async fn check(&self, path: &Path) -> Result<Vec<Diagnostic>, StancError> {
        let source = tokio::fs::read_to_string(path)
            .await
            .map_err(StancError::Io)?;
        let output = self
            .command()
            .arg(path)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output();
        let output = timeout(self.timeout, output)
            .await
            .map_err(|_| StancError::Timeout)?
            .map_err(StancError::Io)?;
        if output.status.success() {
            return Ok(Vec::new());
        }
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_owned();
        let message = if stderr.is_empty() { stdout } else { stderr };
        Ok(parse_stanc_diagnostics(&message, &source))
    }

    pub async fn check_source(
        &self,
        original_path: &Path,
        source: &str,
    ) -> Result<Vec<Diagnostic>, StancError> {
        static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);
        let directory = original_path.parent().unwrap_or_else(|| Path::new("."));
        let mut temporary = None;
        for _ in 0..16 {
            let id = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
            let candidate = directory.join(format!(".stan-lsp-{}-{id}.stan", std::process::id()));
            match std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&candidate)
            {
                Ok(mut file) => {
                    file.write_all(source.as_bytes()).map_err(StancError::Io)?;
                    temporary = Some(candidate);
                    break;
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(StancError::Io(error)),
            }
        }
        let temporary = temporary.ok_or_else(|| {
            StancError::Io(std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                "could not allocate temporary Stan file",
            ))
        })?;
        let result = self.check(&temporary).await;
        if let Err(error) = std::fs::remove_file(&temporary) {
            tracing::warn!(path = %temporary.display(), %error, "could not remove temporary Stan file");
        }
        result
    }

    fn command(&self) -> Command {
        Command::new(&self.executable)
    }
}

fn find_on_path() -> Option<PathBuf> {
    let name = if cfg!(windows) { "stanc.exe" } else { "stanc" };
    env::var_os("PATH")
        .into_iter()
        .flat_map(|paths| env::split_paths(&paths).collect::<Vec<_>>())
        .map(|directory| directory.join(name))
        .find(|path| path.is_file())
}

fn find_cmdstan() -> Option<PathBuf> {
    let name = if cfg!(windows) { "stanc.exe" } else { "stanc" };
    if let Some(path) = env::var_os("CMDSTAN")
        .map(PathBuf::from)
        .map(|root| root.join("bin").join(name))
        .filter(|path| path.is_file())
    {
        return Some(path);
    }
    let root = env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })
        .map(PathBuf::from)?
        .join(".cmdstan");
    let mut candidates = std::fs::read_dir(root)
        .ok()?
        .flatten()
        .map(|entry| entry.path().join("bin").join(name))
        .filter(|path| path.is_file())
        .collect::<Vec<_>>();
    candidates.sort();
    candidates.pop()
}

fn diagnostic_range(message: &str, source: &str) -> TextRange {
    let line = number_after(message, "line ").and_then(|line| line.checked_sub(1));
    let column = number_after(message, "column ").and_then(|column| column.checked_sub(1));
    let (Some(line), Some(column)) = (line, column) else {
        return TextRange::new(0, 0);
    };
    let line_start = source
        .split_inclusive('\n')
        .take(line)
        .map(str::len)
        .sum::<usize>();
    let line_text = source[line_start..].split('\n').next().unwrap_or_default();
    let relative = line_text
        .char_indices()
        .nth(column)
        .map_or(line_text.len(), |(offset, _)| offset);
    let start = line_start + relative;
    let end = source[start..]
        .chars()
        .next()
        .map_or(start, |character| start + character.len_utf8());
    TextRange::new(start, end)
}

fn parse_stanc_diagnostics(message: &str, source: &str) -> Vec<Diagnostic> {
    let blocks = message
        .split("\n\n")
        .map(str::trim)
        .filter(|block| !block.is_empty())
        .collect::<Vec<_>>();
    let blocks = if blocks.is_empty() {
        vec![message]
    } else {
        blocks
    };
    blocks
        .into_iter()
        .map(|block| {
            let warning = block
                .lines()
                .next()
                .is_some_and(|line| line.to_ascii_lowercase().contains("warning"));
            Diagnostic {
                code: DiagnosticCode(if warning {
                    "stanc3.warning"
                } else {
                    "stanc3.compiler"
                }),
                severity: if warning {
                    Severity::Warning
                } else {
                    Severity::Error
                },
                message: block.to_owned(),
                primary_range: diagnostic_range(block, source),
                related: Vec::new(),
                fixes: Vec::new(),
                source: DiagnosticSource::Stanc3,
            }
        })
        .collect()
}

fn number_after(text: &str, marker: &str) -> Option<usize> {
    let rest = text.split_once(marker)?.1;
    rest.chars()
        .take_while(char::is_ascii_digit)
        .collect::<String>()
        .parse()
        .ok()
}

#[derive(Debug, thiserror::Error)]
pub enum StancError {
    #[error("stanc invocation timed out")]
    Timeout,
    #[error("could not run stanc: {0}")]
    Io(std::io::Error),
    #[error("stanc failed: {0}")]
    Failed(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compiler_locations_map_to_utf8_byte_ranges() {
        let source = "data {\n  real θ;\n}\n";
        let range = diagnostic_range("Syntax error, line 2, column 8", source);
        assert_eq!(&source[range.start as usize..range.end as usize], "θ");
    }

    #[test]
    fn multiple_compiler_messages_remain_separate() {
        let diagnostics = parse_stanc_diagnostics(
            "Syntax error, line 1, column 1\n\nWarning, line 2, column 1",
            "x\ny\n",
        );
        assert_eq!(diagnostics.len(), 2);
        assert_eq!(diagnostics[1].severity, Severity::Warning);
    }
}
