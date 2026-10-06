//! Database handle and query wrapper for MesoDB

use crate::rules::STANDARD_RULES;
use mesodb_core::MesoDB;
use std::path::Path;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum DbError {
    #[error("MesoDB internal error: {0}")]
    MesoDb(String),
}

pub struct AstrolabeDb {
    inner: MesoDB,
}

impl AstrolabeDb {
    /// Opens or creates an Astrolabe database at the specified path.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, DbError> {
        let inner = MesoDB::open(path).map_err(|e| DbError::MesoDb(e.to_string()))?;
        Ok(Self { inner })
    }

    /// Access the underlying MesoDB engine directly.
    #[must_use]
    pub fn inner(&self) -> &MesoDB {
        &self.inner
    }

    /// Exposes standard rules for caller-constructed queries.
    #[must_use]
    pub fn standard_rules() -> &'static str {
        STANDARD_RULES
    }

    /// Evaluates a raw Datalog query string directly without rule transformations.
    pub async fn query_json(&self, query_str: &str) -> Result<String, DbError> {
        self.inner
            .query_json(query_str)
            .await
            .map_err(|e| DbError::MesoDb(e.to_string()))
    }

    /// Evaluates a Datalog query, automatically injecting `STANDARD_RULES` if rule
    /// predicates or rule bindings (`%`) are detected.
    pub async fn query_with_rules(&self, query_str: &str) -> Result<String, DbError> {
        self.query_with_custom_rules(query_str, STANDARD_RULES)
            .await
    }

    /// Evaluates a Datalog query with custom recursive rules provided as an EDN string.
    pub async fn query_with_custom_rules(
        &self,
        query_str: &str,
        rules_str: &str,
    ) -> Result<String, DbError> {
        let trimmed = query_str.trim();

        // If the query uses rule terms but hasn't explicitly specified `:in $ %`,
        // auto-inject `:in $ %` before `:where`
        let prepared_query =
            if !trimmed.contains(":in") && (trimmed.contains('%') || contains_rule_call(trimmed)) {
                if let Some(pos) = trimmed.find(":where") {
                    let (head, tail) = trimmed.split_at(pos);
                    format!("{head} :in $ % {tail}")
                } else {
                    trimmed.to_string()
                }
            } else {
                trimmed.to_string()
            };

        // If rule binding `%` is active, bundle query + rules into a multi-input array payload
        let final_payload = if prepared_query.contains('%') {
            format!("[{prepared_query} {rules_str}]")
        } else {
            prepared_query
        };

        self.inner
            .query_json(&final_payload)
            .await
            .map_err(|e| DbError::MesoDb(e.to_string()))
    }
}

/// Helper to detect if a query string invokes standard recursive rule predicates.
#[must_use]
fn contains_rule_call(query: &str) -> bool {
    query.contains("calls?")
        || query.contains("calls-transitive?")
        || query.contains("parent?")
        || query.contains("parent-transitive?")
}
