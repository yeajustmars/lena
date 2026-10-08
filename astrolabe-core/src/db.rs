//! Database handle and query wrapper for MesoDB

use mesodb_core::{
    config::Config,
    db::{MesoDb, OutputFormat, QueryOptions},
};
use std::path::Path;
use thiserror::Error;

use crate::rules::STANDARD_RULES;
use crate::schema;

#[derive(Error, Debug)]
pub enum DbError {
    #[error("MesoDB internal error: {0}")]
    MesoDb(String),
}

pub struct AstrolabeDb {
    inner: MesoDb,
}

impl AstrolabeDb {
    /// Opens or creates an Astrolabe database at the specified path.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, DbError> {
        let inner = MesoDb::open(path, schema::bootstrap(), Config::default())
            .map_err(|e| DbError::MesoDb(e.to_string()))?;
        Ok(Self { inner })
    }

    /// Access the underlying MesoDB engine directly.
    #[must_use]
    pub fn inner(&self) -> &MesoDb {
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

        // MesoDb v0.2.0 requires rules to be passed via QueryOptions
        let options = QueryOptions {
            rules: Some(rules_str.to_string()),
            format: OutputFormat::Json,
            ..Default::default()
        };

        self.inner
            .query_json_with_options(&prepared_query, options)
            .await
            .map_err(|e| DbError::MesoDb(e.to_string()))
    }
}

/// Helper to detect if a query string invokes standard recursive rule predicates.
#[must_use]
fn contains_rule_call(query: &str) -> bool {
    query.contains("calls_rule")
        || query.contains("calls_transitive")
        || query.contains("parent_rule")
        || query.contains("parent_transitive")
}

#[cfg(test)]
mod tests {
    use super::*;
    use mesodb_core::transactor::Fact;
    use mesodb_core::types::Value;
    use tempfile::tempdir;

    async fn setup_db() -> AstrolabeDb {
        let dir = tempdir().unwrap();
        AstrolabeDb::open(dir.path().join("test_db.db")).unwrap()
    }

    #[test]
    fn test_contains_rule_call() {
        assert!(contains_rule_call("[:find ?e :where (calls_rule ?a ?b)]"));
        assert!(contains_rule_call("(parent_transitive ?p ?c)"));
        assert!(!contains_rule_call(
            "[:find ?e :where [?e :entity/type :type/module]]"
        ));
    }

    #[tokio::test]
    async fn test_astrolabedb_open_and_rules() {
        let _db = setup_db().await;
        let rules = AstrolabeDb::standard_rules();
        assert!(rules.contains("calls_transitive"));
        assert!(rules.contains("parent_rule"));
    }

    #[tokio::test]
    async fn test_query_json_basic() {
        let db = setup_db().await;

        db.inner()
            .transact(vec![Fact {
                e: 1,
                ident: ":symbol/name".into(),
                v: Value::String("main_func".into()),
                op: true,
                cas_old_v: None,
                valid_time: None,
            }])
            .await
            .unwrap();

        let query = "[:find ?name :where [1 :symbol/name ?name]]";
        let res = db.query_json(query).await.unwrap();

        assert!(res.contains("main_func"));
    }

    #[tokio::test]
    async fn test_query_with_rules_auto_injects() {
        let db = setup_db().await;

        db.inner()
            .transact(vec![Fact {
                e: 200,
                ident: ":parent/entity".into(),
                v: Value::Ref(100),
                op: true,
                cas_old_v: None,
                valid_time: None,
            }])
            .await
            .unwrap();

        let query = "[:find ?child :where (parent_rule 100 ?child)]";
        let res = db.query_with_rules(query).await.unwrap();

        assert!(res.contains("200"), "Should auto-resolve standard rules");
    }

    #[tokio::test]
    async fn test_query_with_custom_rules() {
        let db = setup_db().await;

        db.inner()
            .transact(vec![
                Fact {
                    e: 1,
                    ident: ":parent/entity".into(),
                    v: Value::Ref(2),
                    op: true,
                    cas_old_v: None,
                    valid_time: None,
                },
                Fact {
                    e: 2,
                    ident: ":parent/entity".into(),
                    v: Value::Ref(3),
                    op: true,
                    cas_old_v: None,
                    valid_time: None,
                },
            ])
            .await
            .unwrap();

        // Note the use of underscores for SQL-safe CTE names
        let custom_rules = r#"
        [
            [(custom_reach ?a ?b) [?a :parent/entity ?b]]
            [(custom_reach ?a ?b) [?a :parent/entity ?c] (custom_reach ?c ?b)]
        ]
        "#;

        let query = "[:find ?target :where (custom_reach 1 ?target)]";
        let res = db
            .query_with_custom_rules(query, custom_rules)
            .await
            .unwrap();

        assert!(res.contains("2"));
        assert!(res.contains("3"));
    }
}
