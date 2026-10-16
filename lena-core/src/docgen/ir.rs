use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocGraph {
    pub project_name: String,
    pub version: String,
    pub languages: BTreeMap<String, DocLanguage>,
    pub search_index: Vec<SearchEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocLanguage {
    pub name: String,
    pub packages: BTreeMap<String, DocPackage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocPackage {
    pub name: String,
    pub language: String,
    pub modules: BTreeMap<String, DocModule>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocModule {
    pub name: String,
    pub path: String,
    pub docstring: Option<String>,
    pub symbols: Vec<DocSymbol>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum SymbolKind {
    Function,
    Method,
    Struct,
    Enum,
    Trait,
    Interface,
    TypeAlias,
    Macro,
    Constant,
    Variable,
    Module,
    Unknown,
}

impl SymbolKind {
    #[must_use]
    pub fn from_str(kind: &str) -> Self {
        match kind.to_lowercase().as_str() {
            "function" | "fn" => Self::Function,
            "method" => Self::Method,
            "struct" => Self::Struct,
            "enum" => Self::Enum,
            "trait" => Self::Trait,
            "interface" => Self::Interface,
            "type" | "typealias" => Self::TypeAlias,
            "macro" => Self::Macro,
            "const" | "constant" => Self::Constant,
            "var" | "variable" => Self::Variable,
            "module" | "mod" => Self::Module,
            _ => Self::Unknown,
        }
    }

    #[must_use]
    pub fn badge_class(&self) -> &'static str {
        match self {
            Self::Function | Self::Method => "badge-fn",
            Self::Struct => "badge-struct",
            Self::Enum => "badge-enum",
            Self::Trait | Self::Interface => "badge-trait",
            Self::TypeAlias => "badge-type",
            Self::Macro => "badge-macro",
            Self::Constant => "badge-const",
            Self::Variable => "badge-var",
            Self::Module => "badge-mod",
            Self::Unknown => "badge-unknown",
        }
    }
}

impl std::fmt::Display for SymbolKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::Function => "Function",
            Self::Method => "Method",
            Self::Struct => "Struct",
            Self::Enum => "Enum",
            Self::Trait => "Trait",
            Self::Interface => "Interface",
            Self::TypeAlias => "Type",
            Self::Macro => "Macro",
            Self::Constant => "Constant",
            Self::Variable => "Variable",
            Self::Module => "Module",
            Self::Unknown => "Unknown",
        };
        write!(f, "{s}")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocSymbol {
    pub fqsn: String,
    pub name: String,
    pub kind: SymbolKind,
    pub signature: Option<String>,
    pub docstring: Option<String>,
    pub file_path: String,
    pub line: Option<usize>,
    pub neighborhood: DocNeighborhood,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DocNeighborhood {
    pub callers: Vec<NeighborRef>,
    pub callees: Vec<NeighborRef>,
    pub mermaid_chart: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NeighborRef {
    pub fqsn: String,
    pub name: String,
    pub file_path: String,
}

impl DocNeighborhood {
    pub fn generate_mermaid(&mut self, current_name: &str) {
        if self.callers.is_empty() && self.callees.is_empty() {
            self.mermaid_chart = None;
            return;
        }

        let mut lines = Vec::new();
        lines.push("graph LR".to_string());
        lines.push(
            "  classDef current fill:#2563eb,stroke:#1d4ed8,stroke-width:2px,color:#fff;"
                .to_string(),
        );
        lines.push(
            "  classDef neighbor fill:#f1f5f9,stroke:#cbd5e1,stroke-width:1px,color:#0f172a;"
                .to_string(),
        );

        lines.push(format!(
            "  node_target[\"{}\"]:::current",
            sanitize_mermaid(current_name)
        ));

        for (idx, caller) in self.callers.iter().enumerate() {
            let caller_id = format!("c_{idx}");
            lines.push(format!(
                "  {}[\"{}\"]:::neighbor --> node_target",
                caller_id,
                sanitize_mermaid(&caller.name)
            ));
        }

        for (idx, callee) in self.callees.iter().enumerate() {
            let callee_id = format!("e_{idx}");
            lines.push(format!(
                "  node_target --> {}[\"{}\"]:::neighbor",
                callee_id,
                sanitize_mermaid(&callee.name)
            ));
        }

        self.mermaid_chart = Some(lines.join("\n"));
    }
}

fn sanitize_mermaid(s: &str) -> String {
    s.replace('"', "#quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('\n', " ")
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchEntry {
    pub fqsn: String,
    pub name: String,
    pub kind: String,
    pub module: String,
    pub summary: String,
    pub href: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_symbol_kind_parsing() {
        assert_eq!(SymbolKind::from_str("function"), SymbolKind::Function);
        assert_eq!(SymbolKind::from_str("FN"), SymbolKind::Function);
        assert_eq!(SymbolKind::from_str("unknown_type"), SymbolKind::Unknown);
    }

    #[test]
    fn test_mermaid_generation_escapes_quotes() {
        let mut neighborhood = DocNeighborhood {
            callers: vec![NeighborRef {
                fqsn: "a".into(),
                name: "caller<T>".into(),
                file_path: "".into(),
            }],
            callees: vec![],
            mermaid_chart: None,
        };
        neighborhood.generate_mermaid("target\"name\"");
        let chart = neighborhood.mermaid_chart.unwrap();
        assert!(chart.contains("target#quot;name#quot;"));
        assert!(chart.contains("caller&lt;T&gt;"));
    }
}
