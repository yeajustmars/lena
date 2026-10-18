//! Hierarchical Configuration Engine for Leña
//!
//! Loads and merges configurations in the following precedence order:
//! 1. Compiled Defaults
//! 2. Global Configuration (`~/.lena/lena.toml`)
//! 3. Project Configuration (`<ROOT>/.lena/lena.toml`)

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use toml::Value;

/// Top-level hierarchical configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct LenaConfig {
    /// General workspace settings
    pub general: GeneralConfig,
    /// Settings controlling `lena prompt` LLM extraction
    pub prompt: PromptConfig,
    /// Settings controlling `lena doc` output
    pub doc: DocConfig,
    /// Adapter installation and lifecycle settings
    pub adapters: AdaptersConfig,
}

impl Default for LenaConfig {
    fn default() -> Self {
        Self {
            general: GeneralConfig::default(),
            prompt: PromptConfig::default(),
            doc: DocConfig::default(),
            adapters: AdaptersConfig::default(),
        }
    }
}

/// General developer-wide settings.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct GeneralConfig {
    /// Desired logging severity
    pub log_level: String,
    /// Location of cached ASTs or DB indices
    pub cache_dir: String,
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            log_level: "info".to_string(),
            cache_dir: "~/.lena/cache".to_string(),
        }
    }
}

/// Token budget controls and feature flags for prompt assembly.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct PromptConfig {
    /// Graph traversal radius
    pub depth: usize,
    /// Include function docstrings in prompt context
    pub include_docs: bool,
    /// Show snippets of where functions are called
    pub call_sites: bool,
    /// Determines which symbols get full implementation source (e.g., "root", "all", "none")
    pub include_source: String,
    /// Regex patterns matching symbol names to include full source
    pub source_patterns: Vec<String>,
    /// Namespace prefixes to include full source
    pub source_namespaces: Vec<String>,
    /// Hard token limit guardrail
    pub max_tokens: Option<usize>,
}

impl Default for PromptConfig {
    fn default() -> Self {
        Self {
            depth: 1,
            include_docs: true,
            call_sites: true,
            include_source: "root".to_string(),
            source_patterns: Vec::new(),
            source_namespaces: Vec::new(),
            max_tokens: None,
        }
    }
}

/// Visual and structural settings for the documentation generator.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct DocConfig {
    /// Structural generator format ("book", "single-page", "mdbook")
    pub format: String,
    /// Relative path to a custom CSS file to inject
    pub custom_css: Option<String>,
    /// Include private/internal symbols in docs
    pub include_private: bool,
    /// Include Mermaid JS call graphs in output
    pub include_call_graphs: bool,
}

impl Default for DocConfig {
    fn default() -> Self {
        Self {
            format: "book".to_string(),
            custom_css: None,
            include_private: false,
            include_call_graphs: true,
        }
    }
}

/// Management configurations for native and foreign adapters.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct AdaptersConfig {
    /// Key-value pair of installed adapters to their target path or "native"
    pub installed: HashMap<String, String>,
}

impl Default for AdaptersConfig {
    fn default() -> Self {
        let mut installed = HashMap::new();
        installed.insert("rust".to_string(), "native".to_string());
        installed.insert("treesitter".to_string(), "native".to_string());
        installed.insert("clj".to_string(), "native".to_string());
        Self { installed }
    }
}

impl LenaConfig {
    /// Loads the configuration by merging Defaults -> Global -> Project.
    ///
    /// # Errors
    /// Returns an error if a found TOML file contains malformed data.
    pub fn load(project_root: Option<&Path>) -> Result<Self> {
        let mut base_value = Value::try_from(Self::default())?;

        // 1. Merge Global Config (~/.lena/lena.toml)
        if let Some(home) = dirs::home_dir() {
            let global_path = home.join(".lena").join("lena.toml");
            if global_path.exists() {
                let global_str = fs::read_to_string(&global_path)?;
                let global_val: Value = toml::from_str(&global_str)?;
                merge_values(&mut base_value, global_val);
            }
        }

        // 2. Merge Project Config (<ROOT>/.lena/lena.toml)
        if let Some(root) = project_root {
            let project_path = root.join(".lena").join("lena.toml");
            if project_path.exists() {
                let project_str = fs::read_to_string(&project_path)?;
                let project_val: Value = toml::from_str(&project_str)?;
                merge_values(&mut base_value, project_val);
            }
        }

        let final_config: Self = base_value.try_into()?;
        Ok(final_config)
    }
}

/// Recursively deep-merges a `toml::Value` override into a base `toml::Value`.
fn merge_values(base: &mut Value, override_val: Value) {
    match (base, override_val) {
        (Value::Table(base_table), Value::Table(override_table)) => {
            for (k, v) in override_table {
                if let Some(base_v) = base_table.get_mut(&k) {
                    if base_v.is_table() && v.is_table() {
                        merge_values(base_v, v);
                    } else {
                        *base_v = v;
                    }
                } else {
                    base_table.insert(k, v);
                }
            }
        }
        (base_val, override_val) => {
            *base_val = override_val;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_default_config_values() {
        let config = LenaConfig::default();
        assert_eq!(config.prompt.depth, 1);
        assert_eq!(config.prompt.include_source, "root");
        assert_eq!(config.doc.format, "book");
        assert!(config.adapters.installed.contains_key("rust"));
    }

    #[test]
    fn test_merge_logic_overrides() {
        let mut base = Value::try_from(LenaConfig::default()).unwrap();

        let override_toml = r#"
        [prompt]
        depth = 3
        include_source = "none"
        max_tokens = 5000

        [doc]
        custom_css = "overridden.css"
        "#;

        let override_val: Value = toml::from_str(override_toml).unwrap();
        merge_values(&mut base, override_val);

        let merged: LenaConfig = base.try_into().unwrap();

        // Ensure overrides applied
        assert_eq!(merged.prompt.depth, 3);
        assert_eq!(merged.prompt.include_source, "none");
        assert_eq!(merged.prompt.max_tokens, Some(5000));
        assert_eq!(merged.doc.custom_css.as_deref(), Some("overridden.css"));

        // Ensure non-overridden fields retained defaults
        assert!(merged.prompt.include_docs);
        assert_eq!(merged.doc.format, "book");
    }

    #[test]
    fn test_config_load_hierarchy() {
        let dir = tempdir().unwrap();
        let project_root = dir.path();

        let lena_dir = project_root.join(".lena");
        fs::create_dir_all(&lena_dir).unwrap();

        let project_toml = r#"
        [general]
        log_level = "debug"

        [prompt]
        source_patterns = ["^test_"]
        "#;
        fs::write(lena_dir.join("lena.toml"), project_toml).unwrap();

        let config = LenaConfig::load(Some(project_root)).unwrap();

        // Assert project overrides took effect
        assert_eq!(config.general.log_level, "debug");
        assert_eq!(config.prompt.source_patterns, vec!["^test_".to_string()]);

        // Assert defaults remained for omitted categories
        assert_eq!(config.prompt.depth, 1);
        assert!(config.adapters.installed.contains_key("rust"));
    }

    #[test]
    fn test_brutal_merge_deep_maps_and_arrays() {
        let mut base = toml::Value::try_from(LenaConfig::default()).unwrap();

        // Simulate a chaotic user TOML overriding arrays, adding new map keys,
        // and overriding existing map keys simultaneously.
        let override_toml = r#"
        [prompt]
        source_patterns = ["^critical_"] # Should completely replace the empty default array

        [adapters.installed]
        python = "/usr/bin/python-adapter" # New key
        rust = "custom-path"               # Override existing key
        "#;

        let override_val: toml::Value = toml::from_str(override_toml).unwrap();
        merge_values(&mut base, override_val);

        let merged: LenaConfig = base.try_into().unwrap();

        // Arrays must be strictly replaced, not concatenated
        assert_eq!(
            merged.prompt.source_patterns,
            vec!["^critical_".to_string()]
        );

        // Maps must be deeply merged (python added, rust overwritten, clj retained)
        assert_eq!(
            merged.adapters.installed.get("python").unwrap(),
            "/usr/bin/python-adapter"
        );
        assert_eq!(
            merged.adapters.installed.get("rust").unwrap(),
            "custom-path"
        );
        assert_eq!(merged.adapters.installed.get("clj").unwrap(), "native");
        assert_eq!(
            merged.adapters.installed.get("treesitter").unwrap(),
            "native"
        );
    }
}
