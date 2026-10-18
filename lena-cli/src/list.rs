//! Symbol Listing and Discovery

#![deny(warnings)]
#![warn(clippy::all, clippy::pedantic)]

use anyhow::Result;
use lena_core::db::LenaDb;
use regex::Regex;
use std::sync::Arc;

pub async fn run(db: &Arc<LenaDb>, filter: Option<&str>) -> Result<()> {
    let query = "[:find ?fqsn :where [_ :symbol/fqsn ?fqsn]]";

    let json_result = db.query_json(query).await?;
    let rows: Vec<serde_json::Value> = serde_json::from_str(&json_result)?;

    let mut fqsns = Vec::new();
    for row in rows {
        let fqsn = if let Some(arr) = row.as_array() {
            arr.first().and_then(|v| v.as_str())
        } else if let Some(obj) = row.as_object() {
            obj.values().next().and_then(|v| v.as_str())
        } else {
            None
        };

        if let Some(f) = fqsn {
            fqsns.push(f.to_string());
        }
    }

    if let Some(pattern) = filter {
        let re = Regex::new(pattern)?;
        fqsns.retain(|f| re.is_match(f));
    }

    fqsns.sort();
    fqsns.dedup();

    if fqsns.is_empty() {
        println!("No symbols found matching the criteria.");
        return Ok(());
    }

    for fqsn in fqsns {
        println!("{fqsn}");
    }

    Ok(())
}
