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
pub struct ServerConfig {
    pub stan_version: String,
    pub stanc_path: Option<PathBuf>,
    pub include_paths: Vec<PathBuf>,
    pub compiler_debounce_ms: u64,
    pub compiler_on_save: bool,
    pub compiler_on_change: bool,
    pub format_indent_width: usize,
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
    pub fn compiler_debounce(&self) -> Duration {
        Duration::from_millis(self.compiler_debounce_ms)
    }

    pub fn formatter_config(&self) -> stan_language::FormatterConfig {
        stan_language::FormatterConfig {
            indent_width: self.format_indent_width.clamp(1, 16),
        }
    }

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
pub struct WorkspaceFile {
    pub path: PathBuf,
    pub text: String,
    pub dependencies: Vec<PathBuf>,
}

#[derive(Debug, Default)]
pub struct Workspace {
    roots: Vec<PathBuf>,
    files: BTreeMap<PathBuf, WorkspaceFile>,
    overlays: BTreeMap<PathBuf, String>,
}

impl Workspace {
    pub fn set_roots(&mut self, roots: Vec<PathBuf>, config: &ServerConfig) {
        self.roots = roots
            .into_iter()
            .map(|root| root.canonicalize().unwrap_or(root))
            .collect();
        self.reindex(config);
    }

    pub fn reindex(&mut self, config: &ServerConfig) {
        self.files.clear();
        let roots = self.roots.clone();
        for root in roots {
            self.scan_directory(&root, config);
        }
    }

    pub fn set_overlay(&mut self, path: PathBuf, text: String, config: &ServerConfig) {
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

    pub fn clear_overlay(&mut self, path: &Path, config: &ServerConfig) {
        self.overlays.remove(path);
        if let Ok(text) = fs::read_to_string(path) {
            let (dependencies, _) = self.resolve_includes(path, &text, config);
            self.files.insert(
                path.to_owned(),
                WorkspaceFile {
                    path: path.to_owned(),
                    text,
                    dependencies,
                },
            );
        }
    }

    pub fn files(&self) -> impl Iterator<Item = &WorkspaceFile> {
        self.files.values()
    }

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
                    diagnostics.push(Diagnostic::error(
                        "workspace.invalid-include",
                        "include directive has no path",
                        TextRange::new(offset, offset + line.len()),
                    ));
                } else if let Some(resolved) = resolve_path(path, requested, &config.include_paths)
                {
                    dependencies.push(resolved);
                } else {
                    diagnostics.push(Diagnostic::error(
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
