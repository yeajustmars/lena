//! Wrapper around `mesodb-core` providing Astrolabe-specific initialization and queries.

use mesodb_core::{config::Config, db::MesoDB, error::MesoError};
use std::path::Path;
use thiserror::Error;

use crate::schema::bootstrap;

#[derive(Error, Debug)]
pub enum AstrolabeDbError {
    #[error("Database error: {0}")]
    Db(#[from] MesoError),
}

/// The core graph database manager for an Astrolabe project.
pub struct AstrolabeDb {
    inner: MesoDB,
}

impl AstrolabeDb {
    /// Opens or creates an Astrolabe database at the given path.
    /// Bootstraps the required UIR schema automatically.
    ///
    /// # Errors
    /// Returns `AstrolabeDbError` if the underlying database fails to open or initialize.
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, AstrolabeDbError> {
        let schema = bootstrap();

        // Use a balanced config by default. Can be extended to accept a user-provided config.
        let config = Config::balanced();

        let inner = MesoDB::open(path, schema, config)?;

        Ok(Self { inner })
    }

    /// Provides read-only access to the underlying `MesoDB` instance for queries.
    #[must_use]
    pub fn inner(&self) -> &MesoDB {
        &self.inner
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[tokio::test]
    async fn test_astrolabe_db_initialization() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("astrolabe.db");

        let db = AstrolabeDb::open(&db_path).expect("Failed to open DB");

        // Verify the database created the required files (WAL, parquet dir, indexes)
        let parent_dir = db_path.parent().unwrap();
        assert!(parent_dir.join("now.idx").exists());
        assert!(parent_dir.join("parquet").exists());

        // Perform a simple smoke test query against the bootstrapped schema
        let query = r#"[:find ?type :where [?e :entity/type ?type]]"#;
        let results = db.inner().query(query).await.expect("Failed basic query");

        assert_eq!(results.len(), 1, "Should return an empty but valid RecordBatch");
        assert_eq!(results[0].num_rows(), 0, "Database should be initially empty");
    }
}
