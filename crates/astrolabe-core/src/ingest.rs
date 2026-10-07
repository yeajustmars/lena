// crates/astrolabe-core/src/ingest.rs
//! ND-EDN Streaming Ingestion Pipeline.

use mesodb_core::edn::parse_transaction;
use std::sync::Arc;
use thiserror::Error;
use tokio::io::{AsyncBufReadExt, BufReader};
use tracing::{error, info, warn};

use crate::db::AstrolabeDb;

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
pub async fn ingest_stream<R>(db: &Arc<AstrolabeDb>, reader: R) -> Result<usize, IngestError>
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
            Ok(facts) => {
                let fact_count = facts.len();
                if fact_count > 0 {
                    // Transact directly into MesoDB
                    match db.inner().transact(facts).await {
                        Ok(report) => {
                            total_facts += report.datoms_written;
                            total_txs += 1;
                        }
                        Err(e) => {
                            error!("Failed to transact batch: {e}");
                            return Err(IngestError::Database(e.to_string()));
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::AstrolabeDb;

    #[tokio::test]
    async fn test_ingest_stream() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("ingest.db");
        let db = Arc::new(AstrolabeDb::open(&db_path).unwrap());

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
}
