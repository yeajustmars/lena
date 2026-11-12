// lena-core/src/ingest.rs

//! ND-EDN Streaming Ingestion Pipeline.

use lena_protocol::validate::{FactValidator, ValidationFact};
use mesodb_core::edn::parse_transaction;
use mesodb_core::types::Value;
use std::sync::Arc;
use thiserror::Error;
use tokio::io::{AsyncBufReadExt, BufReader};
use tracing::{debug, error, warn};

use crate::db::LenaDb;

#[derive(Error, Debug)]
pub enum IngestError {
    #[error("I/O error during ingestion: {0}")]
    Io(#[from] std::io::Error),
    #[error("Database transaction error: {0}")]
    Database(String),
}

/// Ingests ND-EDN lines from an asynchronous reader (like an adapter's stdout)
/// and streams them directly into the database, validating them against the protocol.
///
/// # Errors
/// Returns `IngestError` if the underlying reader fails.
pub async fn ingest_stream<R>(
    db: &Arc<LenaDb>,
    reader: R,
    allow_incomplete_facts: bool,
    dry_run: bool,
    adapter_name: &str,
) -> Result<usize, IngestError>
where
    R: tokio::io::AsyncRead + Unpin + Send + 'static,
{
    let mut buf_reader = BufReader::new(reader);
    let mut line = String::new();
    let mut total_facts = 0;
    let mut total_txs = 0;

    let validator = FactValidator::new(allow_incomplete_facts);

    loop {
        line.clear();
        let bytes_read = buf_reader.read_line(&mut line).await?;
        if bytes_read == 0 {
            break; // EOF
        }

        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        match parse_transaction(trimmed) {
            Ok(raw_facts) => {
                // 1. Prepare facts for validation
                let mut string_cache = Vec::with_capacity(raw_facts.len());
                let mut val_facts = Vec::with_capacity(raw_facts.len());

                for fact in &raw_facts {
                    // Extract strings for the validator to check delimiters/extensions
                    let val_str = match &fact.v {
                        Value::String(s) => s.clone(),
                        _ => fact.v.to_string(),
                    };
                    string_cache.push(val_str);
                }

                for (fact, cached_str) in raw_facts.iter().zip(string_cache.iter()) {
                    val_facts.push(ValidationFact {
                        e: fact.e,
                        ident: &fact.ident,
                        value_str: cached_str,
                    });
                }

                // 2. Validate Batch
                if let Err(errors) = validator.validate_batch(&val_facts) {
                    for err in errors {
                        error!("[{}] Validation Error: {}", adapter_name, err);
                    }
                    error!("[{}] Skipping malformed transaction batch.", adapter_name);
                    continue;
                }

                // 3. Sanitize and Transact
                let facts = sanitize_batch(raw_facts);
                let fact_count = facts.len();
                if fact_count > 0 {
                    if dry_run {
                        // In dry-run mode, we count the facts that *would* have been transacted
                        total_facts += fact_count;
                        total_txs += 1;
                    } else {
                        match db.inner().transact(facts).await {
                            Ok(report) => {
                                total_facts += report.datoms_written;
                                total_txs += 1;
                            }
                            Err(e) => {
                                error!("Failed to transact batch (skipping): {e}");
                            }
                        }
                    }
                }
            }
            Err(e) => {
                warn!("Failed to parse EDN transaction line: {e}");
            }
        }
    }

    debug!("Ingestion complete. Transacted {total_facts} facts across {total_txs} transactions.");
    Ok(total_facts)
}

/// Intercepts and sanitizes a batch of facts before transacting them into MesoDB.
fn sanitize_batch(facts: Vec<mesodb_core::transactor::Fact>) -> Vec<mesodb_core::transactor::Fact> {
    use std::collections::{HashMap, HashSet};

    let mut fqsn_to_e = HashMap::new();
    let mut e_replacements = HashMap::new();

    // 1. Identify entity ID collisions based on the unique `:symbol/fqsn`
    for fact in &facts {
        if (fact.ident == ":symbol/fqsn" || fact.ident == "symbol/fqsn")
            && let Value::String(s) = &fact.v
        {
            if let Some(&target_e) = fqsn_to_e.get(s) {
                if target_e != fact.e {
                    e_replacements.insert(fact.e, target_e);
                }
            } else {
                fqsn_to_e.insert(s.clone(), fact.e);
            }
        }
    }

    let mut unique_fingerprints = HashSet::new();
    let mut sanitized = Vec::new();

    // 2. Apply replacements and deduplicate
    for mut fact in facts {
        let mut current_e = fact.e;
        while let Some(&target_e) = e_replacements.get(&current_e) {
            if current_e == target_e {
                break;
            }
            current_e = target_e;
        }
        fact.e = current_e;

        if let Value::Ref(r) = fact.v {
            let mut current_r = r;
            while let Some(&target_r) = e_replacements.get(&current_r) {
                if current_r == target_r {
                    break;
                }
                current_r = target_r;
            }
            fact.v = Value::Ref(current_r);
        }

        let fingerprint = format!("{}:{}:{:?}:{}", fact.e, fact.ident, fact.v, fact.op);
        if unique_fingerprints.insert(fingerprint) {
            sanitized.push(fact);
        }
    }

    sanitized
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::LenaDb;

    async fn setup_db() -> Arc<LenaDb> {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("ingest.db");
        Arc::new(LenaDb::open(&db_path).unwrap())
    }

    #[tokio::test]
    async fn test_ingest_stream_basic_permissive() {
        let db = setup_db().await;
        // Incomplete facts (missing mandatory module/file paths), so we test in permissive mode
        let edn_stream = b"[[:db/add 100 :entity/type :type/module] [:db/add 100 :symbol/name \"my_mod\"]]\n[[:db/add 101 :entity/type :type/function]]\n";

        let facts_written = ingest_stream(&db, &edn_stream[..], true, false, "test")
            .await
            .expect("Ingestion failed");
        assert_eq!(facts_written, 3);

        let results = db
            .inner()
            .query("[:find ?e :where [?e :entity/type :type/function]]")
            .await
            .unwrap();
        assert_eq!(results[0].num_rows(), 1);
    }

    #[tokio::test]
    async fn test_ingest_stream_strict_rejection() {
        let db = setup_db().await;
        // Incomplete fact violating strict validation (missing FQSN, LOC_FILE, etc.)
        let edn_stream = b"[[:db/add 100 :symbol/name \"my_mod\"]]\n";

        let facts_written = ingest_stream(&db, &edn_stream[..], false, false, "test")
            .await
            .expect("Ingestion failed");

        // The transaction should be skipped because it failed validation
        assert_eq!(facts_written, 0);
    }

    #[tokio::test]
    async fn test_ingest_stream_skips_empty_lines() {
        let db = setup_db().await;
        let edn_stream = b"\n   \n\t\n[[:db/add 100 :symbol/name \"test\"]]\n\n";

        let facts_written = ingest_stream(&db, &edn_stream[..], true, false, "test")
            .await
            .expect("Ingestion failed");
        assert_eq!(facts_written, 1);
    }

    #[tokio::test]
    async fn test_ingest_stream_resilience_malformed_edn() {
        let db = setup_db().await;
        let edn_stream = b"[[:db/add 100 :symbol/name \"valid_1\"]]\n[MALFORMED NONSENSE\n[[:db/add 101 :symbol/name \"valid_2\"]]\n";

        let facts_written = ingest_stream(&db, &edn_stream[..], true, false, "test")
            .await
            .expect("Ingestion failed");
        assert_eq!(
            facts_written, 2,
            "Should have skipped the malformed line and processed the valid ones"
        );
    }

    #[test]
    fn test_brutal_sanitize_batch_collisions() {
        use mesodb_core::transactor::Fact;
        use mesodb_core::types::Value;

        let raw_batch = vec![
            Fact {
                e: 100,
                ident: ":symbol/fqsn".into(),
                v: Value::String("rust:dup".into()),
                op: true,
                cas_old_v: None,
                valid_time: None,
            },
            Fact {
                e: 101,
                ident: ":symbol/fqsn".into(),
                v: Value::String("rust:dup".into()),
                op: true,
                cas_old_v: None,
                valid_time: None,
            },
            Fact {
                e: 102,
                ident: ":symbol/fqsn".into(),
                v: Value::String("rust:dup".into()),
                op: true,
                cas_old_v: None,
                valid_time: None,
            },
            Fact {
                e: 200,
                ident: ":call/callee".into(),
                v: Value::Ref(101),
                op: true,
                cas_old_v: None,
                valid_time: None,
            },
            Fact {
                e: 200,
                ident: ":call/callee".into(),
                v: Value::Ref(102),
                op: true,
                cas_old_v: None,
                valid_time: None,
            },
        ];

        let sanitized = sanitize_batch(raw_batch);

        let fqsn_facts: Vec<_> = sanitized
            .iter()
            .filter(|f| f.ident == ":symbol/fqsn")
            .collect();
        assert_eq!(
            fqsn_facts.len(),
            1,
            "Should mathematically collapse identical FQSN assertions"
        );

        let call_facts: Vec<_> = sanitized
            .iter()
            .filter(|f| f.ident == ":call/callee")
            .collect();
        assert_eq!(
            call_facts.len(),
            1,
            "Should collapse identical remapped references"
        );
    }

    #[tokio::test]
    async fn test_ingest_stream_strict_acceptance() {
        let db = setup_db().await;
        let edn_stream = b"[[:db/add 100 :entity/type :type/function] [:db/add 100 :symbol/fqsn \"rust:core::test\"] [:db/add 100 :symbol/name \"test\"] [:db/add 100 :symbol/module \"core\"] [:db/add 100 :location/file \"src/lib.rs\"]]\n";

        let facts_written = ingest_stream(&db, &edn_stream[..], false, false, "test")
            .await
            .expect("Ingestion failed");
        assert_eq!(facts_written, 5);

        let results = db
            .inner()
            .query("[:find ?name :where [100 :symbol/name ?name]]")
            .await
            .unwrap();
        assert_eq!(results[0].num_rows(), 1);
    }

    #[tokio::test]
    async fn test_ingest_stream_strict_rejects_module_with_extension() {
        let db = setup_db().await;
        // Module contains .rs extension -> must be rejected under strict mode
        let edn_stream = b"[[:db/add 100 :entity/type :type/function] [:db/add 100 :symbol/fqsn \"rust:core::test\"] [:db/add 100 :symbol/name \"test\"] [:db/add 100 :symbol/module \"core.rs\"] [:db/add 100 :location/file \"src/lib.rs\"]]\n";

        let facts_written = ingest_stream(&db, &edn_stream[..], false, false, "test")
            .await
            .expect("Ingestion execution errored");
        assert_eq!(
            facts_written, 0,
            "Batch with module file extension must be rejected"
        );
    }
}
