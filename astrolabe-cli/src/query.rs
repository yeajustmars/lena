// astrolabe-cli/src/query.rs

use anyhow::Result;
use astrolabe_core::db::AstrolabeDb;
use std::sync::Arc;

pub async fn run(db: &Arc<AstrolabeDb>, query: &str) -> Result<()> {
    match db.query_with_rules(query).await {
        Ok(json) => println!("{json}"),
        Err(e) => eprintln!("Query error: {e}"),
    }
    Ok(())
}
