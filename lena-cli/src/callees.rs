// lena-cli/src/callees.rs

#![deny(warnings)]
#![warn(clippy::all, clippy::pedantic)]

use anyhow::Result;
use lena_core::db::LenaDb;
use std::sync::Arc;

pub async fn run(db: &Arc<LenaDb>, fqsn: &str) -> Result<()> {
    let query = format!(
        "[:find ?callee_fqsn
          :where [?caller :symbol/fqsn \"{fqsn}\"]
                 (calls_rule ?caller ?callee)
                 [?callee :symbol/fqsn ?callee_fqsn]]"
    );
    match db.query_with_rules(&query).await {
        Ok(json) => println!("{json}"),
        Err(e) => eprintln!("Query error: {e}"),
    }
    Ok(())
}
