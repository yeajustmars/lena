// lena-cli/src/callers.rs

#![deny(warnings)]
#![warn(clippy::all, clippy::pedantic)]

use anyhow::Result;
use lena_core::db::LenaDb;
use std::sync::Arc;

pub async fn run(db: &Arc<LenaDb>, fqsn: &str) -> Result<()> {
    let query = format!(
        "[:find ?caller_fqsn
          :where [?callee :symbol/fqsn \"{fqsn}\"]
                 (calls_rule ?caller ?callee)
                 [?caller :symbol/fqsn ?caller_fqsn]]"
    );
    match db.query_with_rules(&query).await {
        Ok(json) => println!("{json}"),
        Err(e) => eprintln!("Query error: {e}"),
    }
    Ok(())
}
