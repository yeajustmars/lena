use crate::config::ReportConfig;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
    time::Duration,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Diagnostic {
    pub level: String,
    pub file: String,
    pub line: i64,
    pub col: i64,
    pub symbol: String,
    pub category: String,
    pub message: String,
    pub adapter: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AdapterSummary {
    pub total_facts: usize,
    pub duration_ms: u64,
    pub diagnostic_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct IndexingReport {
    pub timestamp: String,
    pub target_dir: String,
    pub total_duration_ms: u64,
    pub total_facts_ingested: usize,
    pub adapter_summaries: std::collections::HashMap<String, AdapterSummary>,
    pub diagnostics: Vec<Diagnostic>,
}

impl IndexingReport {
    pub fn new(target: &Path) -> Self {
        Self {
            timestamp: chrono::Utc::now().to_rfc3339(),
            target_dir: target.to_string_lossy().to_string(),
            ..Default::default()
        }
    }

    pub fn record_diagnostic(&mut self, diag: Diagnostic) {
        if let Some(summary) = self.adapter_summaries.get_mut(&diag.adapter) {
            summary.diagnostic_count += 1;
        } else {
            let mut summary = AdapterSummary::default();
            summary.diagnostic_count = 1;
            self.adapter_summaries.insert(diag.adapter.clone(), summary);
        }
        self.diagnostics.push(diag);
    }

    pub fn record_adapter_success(&mut self, adapter: &str, facts: usize, duration: Duration) {
        let summary = self
            .adapter_summaries
            .entry(adapter.to_string())
            .or_default();
        summary.total_facts += facts;
        summary.duration_ms += duration.as_millis() as u64;
    }

    pub fn write_to_disk(
        &self,
        config: &ReportConfig,
        project_root: &Path,
    ) -> anyhow::Result<PathBuf> {
        if !config.enabled {
            return Ok(PathBuf::new());
        }

        let out_dir = project_root.join(&config.output_dir);
        fs::create_dir_all(&out_dir)?;

        let ts = chrono::Utc::now().format("%Y%m%d-%H%M%S");
        let ext = match config.format.to_lowercase().as_str() {
            "json" | "ndjson" => "json",
            "edn" | "ndedn" => "edn",
            "markdown" => "md",
            "xml" => "xml",
            _ => "log",
        };

        let file_path = out_dir.join(format!("index-{}.{}", ts, ext));
        let content = self.format_report(&config.format);
        fs::write(&file_path, content)?;

        Ok(file_path)
    }

    fn format_report(&self, format: &str) -> String {
        match format.to_lowercase().as_str() {
            "json" => serde_json::to_string_pretty(self).unwrap_or_default(),
            "ndjson" => {
                let mut out = String::new();
                out.push_str(
                    &serde_json::to_string(&serde_json::json!({
                        "meta": {
                            "timestamp": self.timestamp,
                            "target_dir": self.target_dir,
                            "total_duration_ms": self.total_duration_ms,
                            "total_facts_ingested": self.total_facts_ingested,
                            "adapter_summaries": self.adapter_summaries
                        }
                    }))
                    .unwrap(),
                );
                out.push('\n');
                for diag in &self.diagnostics {
                    out.push_str(&serde_json::to_string(diag).unwrap());
                    out.push('\n');
                }
                out
            }
            "edn" | "ndedn" => {
                // Highly simplified EDN formatter for Clojure ecosystem consumers
                let mut out = format!(
                    "{{:timestamp \"{}\" :target-dir \"{}\" :total-duration-ms {} :total-facts {} :adapters {{",
                    self.timestamp,
                    self.target_dir,
                    self.total_duration_ms,
                    self.total_facts_ingested
                );

                for (adapter, summary) in &self.adapter_summaries {
                    out.push_str(&format!(
                        ":{} {{:facts {} :duration-ms {} :diagnostics {}}} ",
                        adapter, summary.total_facts, summary.duration_ms, summary.diagnostic_count
                    ));
                }
                out.push_str("}\n:diagnostics [\n");

                for diag in &self.diagnostics {
                    out.push_str(&format!("  {{:adapter \"{}\" :level :{} :file \"{}\" :line {} :col {} :symbol \"{}\" :category :{} :message \"{}\"}}\n",
                        diag.adapter, diag.level.to_lowercase(), diag.file, diag.line, diag.col, diag.symbol, diag.category, diag.message.replace('"', "\\\"")));
                }
                out.push_str("]}\n");
                out
            }
            "markdown" => {
                let mut out = format!(
                    "# Leña Indexing Report\n\n**Target:** `{}`  \n**Timestamp:** `{}`  \n**Total Time:** `{}ms`  \n**Total Facts:** `{}`\n\n## Adapter Summary\n\n| Adapter | Facts | Duration (ms) | Diagnostics |\n|---|---|---|---|\n",
                    self.target_dir,
                    self.timestamp,
                    self.total_duration_ms,
                    self.total_facts_ingested
                );

                for (adapter, summary) in &self.adapter_summaries {
                    out.push_str(&format!(
                        "| {} | {} | {} | {} |\n",
                        adapter, summary.total_facts, summary.duration_ms, summary.diagnostic_count
                    ));
                }

                out.push_str("\n## Diagnostics\n\n");
                for diag in &self.diagnostics {
                    out.push_str(&format!(
                        "* **{}** `[{}]` in `{}:{}:{}` ({}): {}\n",
                        diag.level.to_uppercase(),
                        diag.adapter,
                        diag.file,
                        diag.line,
                        diag.col,
                        diag.symbol,
                        diag.message
                    ));
                }
                out
            }
            "xml" => {
                let mut out = format!(
                    "<indexing-report>\n  <meta target=\"{}\" timestamp=\"{}\" duration-ms=\"{}\" facts=\"{}\"/>\n  <adapters>\n",
                    self.target_dir,
                    self.timestamp,
                    self.total_duration_ms,
                    self.total_facts_ingested
                );

                for (adapter, summary) in &self.adapter_summaries {
                    out.push_str(&format!("    <adapter name=\"{}\" facts=\"{}\" duration-ms=\"{}\" diagnostics=\"{}\"/>\n",
                        adapter, summary.total_facts, summary.duration_ms, summary.diagnostic_count));
                }
                out.push_str("  </adapters>\n  <diagnostics>\n");

                for diag in &self.diagnostics {
                    out.push_str(&format!("    <diagnostic level=\"{}\" adapter=\"{}\" file=\"{}\" line=\"{}\" col=\"{}\" symbol=\"{}\" category=\"{}\">{}</diagnostic>\n",
                        diag.level, diag.adapter, diag.file, diag.line, diag.col, diag.symbol, diag.category, diag.message));
                }
                out.push_str("  </diagnostics>\n</indexing-report>\n");
                out
            }
            _ => {
                // Standard LOG format
                let mut out = format!(
                    "LEÑA INDEXING REPORT\nTarget: {}\nTimestamp: {}\nDuration: {}ms\nFacts: {}\n\nADAPTERS:\n",
                    self.target_dir,
                    self.timestamp,
                    self.total_duration_ms,
                    self.total_facts_ingested
                );

                for (adapter, summary) in &self.adapter_summaries {
                    out.push_str(&format!(
                        "  [{}] Facts: {} | Duration: {}ms | Diagnostics: {}\n",
                        adapter, summary.total_facts, summary.duration_ms, summary.diagnostic_count
                    ));
                }

                out.push_str("\nDIAGNOSTICS:\n");
                for diag in &self.diagnostics {
                    out.push_str(&format!(
                        "[{}] {} {}:{}:{} ({}): {} - {}\n",
                        diag.adapter,
                        diag.level.to_uppercase(),
                        diag.file,
                        diag.line,
                        diag.col,
                        diag.symbol,
                        diag.category,
                        diag.message
                    ));
                }
                out
            }
        }
    }
}
