// astrolabe-cli/src/callees.rs

#![deny(warnings)]
#![warn(clippy::all, clippy::pedantic)]

use anyhow::Result;
use astrolabe_core::db::AstrolabeDb;
use std::sync::Arc;

pub async fn run(db: &Arc<AstrolabeDb>, fqsn: &str) -> Result<()> {
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
