// lena-cli/src/query.rs

#![deny(warnings)]
#![warn(clippy::all, clippy::pedantic)]

use anyhow::Result;
use lena_core::db::LenaDb;
use std::sync::Arc;

pub async fn run(db: &Arc<LenaDb>, query: &str) -> Result<()> {
    match db.query_with_rules(query).await {
        Ok(json) => println!("{json}"),
        Err(e) => eprintln!("Query error: {e}"),
    }
    Ok(())
}
