// crates/lena-core/src/ingest.rs
//! ND-EDN Streaming Ingestion Pipeline.

use mesodb_core::edn::parse_transaction;
use std::sync::Arc;
use thiserror::Error;
use tokio::io::{AsyncBufReadExt, BufReader};
use tracing::{error, info, warn};

use crate::db::LenaDb;

#[derive(Error, Debug)]
pub enum IngestError {
    #[error("I/O error during ingestion: {0}")]
    Io(#[from] std::io::Error),
    #[error("Database transaction error: {0}")]
    Database(String),
}

/// Ingests ND-EDN lines from an asynchronous reader (like an adapter's stdout)
/// and streams them directly into the database.
///
/// # Errors
/// Returns `IngestError` if the underlying reader fails or if a transaction panics.
pub async fn ingest_stream<R>(db: &Arc<LenaDb>, reader: R) -> Result<usize, IngestError>
where
    R: tokio::io::AsyncRead + Unpin + Send + 'static,
{
    let mut buf_reader = BufReader::new(reader);
    let mut line = String::new();
    let mut total_facts = 0;
    let mut total_txs = 0;

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

        // Parse the EDN transaction array
        match parse_transaction(trimmed) {
            Ok(raw_facts) => {
                let facts = sanitize_batch(raw_facts);
                let fact_count = facts.len();
                if fact_count > 0 {
                    // Transact directly into MesoDB
                    match db.inner().transact(facts).await {
                        Ok(report) => {
                            total_facts += report.datoms_written;
                            total_txs += 1;
                        }
                        Err(e) => {
                            // Log and skip malformed transactions without crashing the whole pipeline
                            error!("Failed to transact batch (skipping): {e}");
                        }
                    }
                }
            }
            Err(e) => {
                // Log and skip malformed lines from adapters without crashing the whole pipeline
                warn!("Failed to parse EDN transaction line: {e}");
            }
        }
    }

    info!("Ingestion complete. Transacted {total_facts} facts across {total_txs} transactions.");
    Ok(total_facts)
}

/// Intercepts and sanitizes a batch of facts before transacting them into MesoDB.
/// 1. Coalesces temporary entity IDs if multiple nodes assert the same unique `:symbol/fqsn`.
/// 2. Remaps all `Value::Ref` edges to point to the coalesced entity IDs.
/// 3. Removes exact duplicate fact assertions to prevent batch conflicts.
fn sanitize_batch(facts: Vec<mesodb_core::transactor::Fact>) -> Vec<mesodb_core::transactor::Fact> {
    use mesodb_core::types::Value;
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
        // Remap subject entity ID
        let mut current_e = fact.e;
        while let Some(&target_e) = e_replacements.get(&current_e) {
            if current_e == target_e {
                break;
            } // prevent infinite loop
            current_e = target_e;
        }
        fact.e = current_e;

        // Remap object entity ID if it's a reference (graph edge)
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

        // Fingerprint to remove exact duplicate assertions
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

    #[tokio::test]
    async fn test_ingest_stream() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("ingest.db");
        let db = Arc::new(LenaDb::open(&db_path).unwrap());

        let edn_stream = b"[[:db/add 100 :entity/type :type/module] [:db/add 100 :symbol/name \"my_mod\"]]\n[[:db/add 101 :entity/type :type/function]]\n";

        let facts_written = ingest_stream(&db, &edn_stream[..])
            .await
            .expect("Ingestion failed");

        // 2 datoms in first tx, 1 in second
        assert_eq!(facts_written, 3);

        // Verify via query
        let results = db
            .inner()
            .query("[:find ?e :where [?e :entity/type :type/function]]")
            .await
            .unwrap();
        assert_eq!(results[0].num_rows(), 1);
    }

    async fn setup_db() -> Arc<LenaDb> {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("ingest.db");
        Arc::new(LenaDb::open(&db_path).unwrap())
    }

    #[tokio::test]
    async fn test_ingest_stream_basic() {
        let db = setup_db().await;
        let edn_stream = b"[[:db/add 100 :entity/type :type/module] [:db/add 100 :symbol/name \"my_mod\"]]\n[[:db/add 101 :entity/type :type/function]]\n";

        let facts_written = ingest_stream(&db, &edn_stream[..])
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
    async fn test_ingest_stream_skips_empty_lines() {
        let db = setup_db().await;
        // Includes empty lines and whitespace-only lines
        let edn_stream = b"\n   \n\t\n[[:db/add 100 :symbol/name \"test\"]]\n\n";

        let facts_written = ingest_stream(&db, &edn_stream[..])
            .await
            .expect("Ingestion failed");
        assert_eq!(facts_written, 1);
    }

    #[tokio::test]
    async fn test_ingest_stream_resilience_malformed_edn() {
        let db = setup_db().await;
        // Line 2 is completely invalid EDN. The stream should skip it and process Line 3.
        let edn_stream = b"[[:db/add 100 :symbol/name \"valid_1\"]]\n[MALFORMED NONSENSE\n[[:db/add 101 :symbol/name \"valid_2\"]]\n";

        let facts_written = ingest_stream(&db, &edn_stream[..])
            .await
            .expect("Ingestion failed");
        assert_eq!(
            facts_written, 2,
            "Should have skipped the malformed line and processed the valid ones"
        );
    }

    #[tokio::test]
    async fn test_ingest_stream_empty_transactions() {
        let db = setup_db().await;
        // Empty transaction arrays should parse but result in 0 facts written
        let edn_stream = b"[]\n[  ]\n[[:db/add 100 :symbol/name \"real\"]]\n";

        let facts_written = ingest_stream(&db, &edn_stream[..])
            .await
            .expect("Ingestion failed");
        assert_eq!(facts_written, 1);
    }

    #[tokio::test]
    async fn test_ingest_stream_clean_eof() {
        let db = setup_db().await;
        // An empty stream should just return 0 without error
        let edn_stream = b"";

        let facts_written = ingest_stream(&db, &edn_stream[..])
            .await
            .expect("Ingestion should cleanly exit on EOF");
        assert_eq!(facts_written, 0);
    }

    #[tokio::test]
    async fn test_ingest_stream_with_escaped_quotes() {
        let db = setup_db().await;

        // This stream simulates an adapter emitting a docstring or signature with embedded quotes and spaces.
        // In older versions of mesodb (pre v0.2.1), the lexer would stop at the first escaped quote,
        // treat the rest of the string as separate tokens, and crash looking for an #inst timestamp.
        let edn_stream =
            b"[[:db/add 100 :code/signature \"fn test() { println!(\\\"Hello\\\"); }\"]]\n";

        let facts_written = ingest_stream(&db, &edn_stream[..])
            .await
            .expect("Ingestion failed on escaped quotes");

        assert_eq!(
            facts_written, 1,
            "Should successfully parse and transact the fact without `#inst` serialization errors"
        );

        // Verify the data was stored correctly
        let results = db
            .inner()
            .query("[:find ?sig :where [100 :code/signature ?sig]]")
            .await
            .unwrap();

        assert_eq!(
            results[0].num_rows(),
            1,
            "The escaped string fact must be retrievable"
        );
    }

    #[test]
    fn test_brutal_sanitize_batch_collisions() {
        use mesodb_core::transactor::Fact;
        use mesodb_core::types::Value;

        // Create a chaotic batch with severe FQSN collisions and duplicate facts
        let raw_batch = vec![
            // ID 100, 101, and 102 all assert the EXACT SAME unique FQSN
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
            // ID 200 calls ID 101 AND 102 (which are aliases of 100)
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
            // Exact duplicate spam for ID 100 to test fingerprint deduplication
            Fact {
                e: 100,
                ident: ":entity/language".into(),
                v: Value::String("Rust".into()),
                op: true,
                cas_old_v: None,
                valid_time: None,
            },
            Fact {
                e: 100,
                ident: ":entity/language".into(),
                v: Value::String("Rust".into()),
                op: true,
                cas_old_v: None,
                valid_time: None,
            },
        ];

        let sanitized = sanitize_batch(raw_batch);

        // 1. Only ONE FQSN assertion should remain
        let fqsn_facts: Vec<_> = sanitized
            .iter()
            .filter(|f| f.ident == ":symbol/fqsn")
            .collect();
        assert_eq!(
            fqsn_facts.len(),
            1,
            "Should mathematically collapse identical FQSN assertions"
        );
        let surviving_e = fqsn_facts[0].e;

        // 2. The edge references to 101 and 102 MUST be remapped to the surviving entity ID,
        // and because they are identical post-remap, they must deduplicate into a single edge.
        let call_facts: Vec<_> = sanitized
            .iter()
            .filter(|f| f.ident == ":call/callee")
            .collect();
        assert_eq!(
            call_facts.len(),
            1,
            "Should collapse identical remapped references"
        );
        assert_eq!(
            call_facts[0].v,
            Value::Ref(surviving_e),
            "Edge must be remapped to surviving entity"
        );

        // 3. Exact duplicates of the language attribute must be reduced to 1
        let lang_facts: Vec<_> = sanitized
            .iter()
            .filter(|f| f.ident == ":entity/language")
            .collect();
        assert_eq!(
            lang_facts.len(),
            1,
            "Should strip exact duplicate assertions via fingerprinting"
        );
    }
}
