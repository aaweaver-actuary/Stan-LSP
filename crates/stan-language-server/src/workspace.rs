//! Folder indexing, configuration, overlays, and Stan include resolution.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
    time::Duration,
};

use serde::Deserialize;
use stan_language::{Diagnostic, TextRange};

#[derive(Debug, Clone, Deserialize)]
#[serde(default, rename_all = "camelCase")]
/// Editor-neutral server settings accepted during initialization and updates.
pub struct ServerConfig {
    /// Embedded Stan catalog version requested by the project.
    pub stan_version: String,
    /// Explicit stanc3 executable, ahead of environment discovery.
    pub stanc_path: Option<PathBuf>,
    /// Additional roots used for relative include resolution.
    pub include_paths: Vec<PathBuf>,
    /// Delay before optional change-triggered compiler validation.
    pub compiler_debounce_ms: u64,
    /// Whether save notifications run stanc3.
    pub compiler_on_save: bool,
    /// Whether changes schedule debounced stanc3 validation.
    pub compiler_on_change: bool,
    /// Native formatter indentation width.
    pub format_indent_width: usize,
    /// Lint ID to level-name overrides.
    pub lint_levels: BTreeMap<String, String>,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            stan_version: stan_language::STAN_VERSION.to_string(),
            stanc_path: None,
            include_paths: Vec::new(),
            compiler_debounce_ms: 500,
            compiler_on_save: true,
            compiler_on_change: false,
            format_indent_width: 2,
            lint_levels: BTreeMap::new(),
        }
    }
}

impl ServerConfig {
    /// Returns the configured compiler debounce duration.
    pub fn compiler_debounce(&self) -> Duration {
        Duration::from_millis(self.compiler_debounce_ms)
    }

    /// Converts server settings to native formatter configuration.
    pub fn formatter_config(&self) -> stan_language::FormatterConfig {
        stan_language::FormatterConfig {
            indent_width: self.format_indent_width.clamp(1, 16),
        }
    }

    /// Converts valid server overrides to shared lint configuration.
    pub fn lint_config(&self) -> stan_language::LintConfig {
        let mut config = stan_language::LintConfig::default();
        for (id, level) in &self.lint_levels {
            let level = match level.as_str() {
                "allow" => stan_language::LintLevel::Allow,
                "hint" => stan_language::LintLevel::Hint,
                "warn" => stan_language::LintLevel::Warn,
                "deny" => stan_language::LintLevel::Deny,
                _ => continue,
            };
            let _ = config.set_named(id, level);
        }
        config
    }
}

#[derive(Debug, Clone)]
/// Indexed workspace source and its resolved include dependencies.
pub struct WorkspaceFile {
    /// Canonical or best-effort absolute path.
    pub path: PathBuf,
    /// Disk source or unsaved overlay text.
    pub text: String,
    /// Resolved direct include dependencies.
    pub dependencies: Vec<PathBuf>,
}

#[derive(Debug, Default)]
/// Folder-based `.stan` index with unsaved-buffer overlays.
pub struct Workspace {
    roots: Vec<PathBuf>,
    files: BTreeMap<PathBuf, WorkspaceFile>,
    overlays: BTreeMap<PathBuf, String>,
}

impl Workspace {
    /// Replaces workspace roots and rebuilds the index.
    pub fn set_roots(&mut self, roots: Vec<PathBuf>, config: &ServerConfig) {
        self.roots = roots
            .into_iter()
            .map(|root| root.canonicalize().unwrap_or(root))
            .collect();
        self.reindex(config);
    }

    /// Rebuilds indexed files while retaining overlays.
    pub fn reindex(&mut self, config: &ServerConfig) {
        self.files.clear();
        let roots = self.roots.clone();
        for root in roots {
            self.scan_directory(&root, config);
        }
    }

    /// Installs unsaved text for a path and refreshes dependencies.
    pub fn set_overlay(&mut self, path: PathBuf, text: String, config: &ServerConfig) {
        let path = path.canonicalize().unwrap_or(path);
        self.overlays.insert(path.clone(), text.clone());
        let (dependencies, _) = self.resolve_includes(&path, &text, config);
        self.files.insert(
            path.clone(),
            WorkspaceFile {
                path,
                text,
                dependencies,
            },
        );
    }

    /// Removes an overlay and reloads disk state when available.
    pub fn clear_overlay(&mut self, path: &Path, config: &ServerConfig) {
        let path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        self.overlays.remove(&path);
        if let Ok(text) = fs::read_to_string(&path) {
            let (dependencies, _) = self.resolve_includes(&path, &text, config);
            self.files.insert(
                path.clone(),
                WorkspaceFile {
                    path,
                    text,
                    dependencies,
                },
            );
        } else {
            self.files.remove(&path);
        }
    }

    /// Iterates indexed files in deterministic path order.
    pub fn files(&self) -> impl Iterator<Item = &WorkspaceFile> {
        self.files.values()
    }

    /// Resolves direct include spellings and returns workspace-owned diagnostics.
    pub fn resolve_includes(
        &self,
        path: &Path,
        text: &str,
        config: &ServerConfig,
    ) -> (Vec<PathBuf>, Vec<Diagnostic>) {
        let mut dependencies = Vec::new();
        let mut diagnostics = Vec::new();
        let mut offset = 0usize;
        for line in text.split_inclusive('\n') {
            let trimmed = line.trim();
            if let Some(rest) = trimmed.strip_prefix("#include") {
                let requested = rest.trim().trim_matches('"');
                if requested.is_empty() {
                    diagnostics.push(workspace_error(
                        "workspace.invalid-include",
                        "include directive has no path",
                        TextRange::new(offset, offset + line.len()),
                    ));
                } else if let Some(resolved) = resolve_path(path, requested, &config.include_paths)
                {
                    dependencies.push(resolved);
                } else {
                    diagnostics.push(workspace_error(
                        "workspace.missing-include",
                        format!("cannot resolve include `{requested}`"),
                        TextRange::new(offset, offset + line.trim_end().len()),
                    ));
                }
            }
            offset += line.len();
        }
        (dependencies, diagnostics)
    }

    /// Returns detected dependency cycles as path sequences.
    pub fn include_cycles(&self) -> Vec<Vec<PathBuf>> {
        let mut cycles = Vec::new();
        let mut visited = BTreeSet::new();
        let mut stack = Vec::new();
        for path in self.files.keys() {
            self.find_cycles(path, &mut visited, &mut stack, &mut cycles);
        }
        cycles
    }

    fn find_cycles(
        &self,
        path: &Path,
        visited: &mut BTreeSet<PathBuf>,
        stack: &mut Vec<PathBuf>,
        cycles: &mut Vec<Vec<PathBuf>>,
    ) {
        if let Some(position) = stack.iter().position(|entry| entry == path) {
            cycles.push(stack[position..].to_vec());
            return;
        }
        if !visited.insert(path.to_owned()) {
            return;
        }
        stack.push(path.to_owned());
        if let Some(file) = self.files.get(path) {
            for dependency in &file.dependencies {
                self.find_cycles(dependency, visited, stack, cycles);
            }
        }
        stack.pop();
    }

    fn scan_directory(&mut self, directory: &Path, config: &ServerConfig) {
        let Ok(entries) = fs::read_dir(directory) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if !matches!(
                    path.file_name().and_then(|name| name.to_str()),
                    Some(".git" | "target" | "node_modules")
                ) {
                    self.scan_directory(&path, config);
                }
            } else if path.extension().and_then(|extension| extension.to_str()) == Some("stan") {
                let text = self
                    .overlays
                    .get(&path)
                    .cloned()
                    .or_else(|| fs::read_to_string(&path).ok());
                if let Some(text) = text {
                    let (dependencies, _) = self.resolve_includes(&path, &text, config);
                    self.files.insert(
                        path.clone(),
                        WorkspaceFile {
                            path,
                            text,
                            dependencies,
                        },
                    );
                }
            }
        }
    }
}

fn workspace_error(code: &'static str, message: impl Into<String>, range: TextRange) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(code, message, range);
    diagnostic.source = stan_language::DiagnosticSource::Workspace;
    diagnostic
}

fn resolve_path(including: &Path, requested: &str, include_paths: &[PathBuf]) -> Option<PathBuf> {
    std::iter::once(including.parent().unwrap_or_else(|| Path::new(".")))
        .chain(include_paths.iter().map(PathBuf::as_path))
        .map(|base| base.join(requested))
        .find(|candidate| candidate.is_file())
        .and_then(|candidate| candidate.canonicalize().ok().or(Some(candidate)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_includes_are_diagnostic_not_panics() {
        let workspace = Workspace::default();
        let (_, diagnostics) = workspace.resolve_includes(
            Path::new("/definitely/missing/model.stan"),
            "#include missing.stan\n",
            &ServerConfig::default(),
        );
        assert_eq!(diagnostics[0].code.0, "workspace.missing-include");
        assert_eq!(
            diagnostics[0].source,
            stan_language::DiagnosticSource::Workspace
        );
    }

    #[test]
    fn indexes_stan_files_and_detects_include_cycles() {
        let root =
            std::env::temp_dir().join(format!("stan-lsp-workspace-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("a.stan"), "#include b.stan\n").unwrap();
        fs::write(root.join("b.stan"), "#include a.stan\n").unwrap();
        let mut workspace = Workspace::default();
        workspace.set_roots(vec![root.clone()], &ServerConfig::default());
        assert_eq!(workspace.files().count(), 2);
        assert!(!workspace.include_cycles().is_empty());
        fs::remove_dir_all(root).unwrap();
    }
}
