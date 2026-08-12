//! Read strategy implementations using enum dispatch.

use std::fmt::Debug;
use std::future::Future;

/// Read strategy enum - determines cache/db access pattern.
#[derive(Debug, Clone, Copy, Default)]
pub enum ReadStrategy {
    /// Check cache first, fall back to DB on miss
    #[default]
    CacheFirst,
    /// Only read from database, skip cache
    DbOnly,
    /// Only read from cache, never hit DB
    CacheOnly,
    /// Read from DB, populate cache on success
    ReadThrough,
}

impl ReadStrategy {
    /// Execute a read operation according to the strategy.
    ///
    /// - `cache_fn`: Async function to read from cache
    /// - `db_fn`: Async function to read from database  
    /// - `populate_fn`: Optional async function to populate cache after DB read
    pub async fn read<T, CacheFut, DbFut, PopulateFut>(
        &self,
        cache_fn: impl FnOnce() -> CacheFut,
        db_fn: impl FnOnce() -> DbFut,
        populate_fn: Option<impl FnOnce(T) -> PopulateFut>,
    ) -> Result<Option<T>, ReadError>
    where
        T: Clone + Debug,
        CacheFut: Future<Output = Result<Option<T>, CacheError>>,
        DbFut: Future<Output = Result<Option<T>, DbError>>,
        PopulateFut: Future<Output = Result<(), CacheError>>,
    {
        match self {
            ReadStrategy::CacheFirst => {
                // Try cache first
                match cache_fn().await {
                    Ok(Some(value)) => {
                        tracing::debug!("Cache hit");
                        return Ok(Some(value));
                    }
                    Ok(None) => {
                        tracing::debug!("Cache miss, falling back to DB");
                    }
                    Err(e) => {
                        tracing::warn!(error = %e, "Cache error, falling back to DB");
                    }
                }

                // Fall back to DB
                let result = db_fn().await.map_err(ReadError::Database)?;
                
                // Populate cache on success
                if let (Some(value), Some(populate)) = (&result, populate_fn) {
                    if let Err(e) = populate(value.clone()).await {
                        tracing::warn!(error = %e, "Failed to populate cache");
                    }
                }
                
                Ok(result)
            }

            ReadStrategy::DbOnly => {
                db_fn().await.map_err(ReadError::Database)
            }

            ReadStrategy::CacheOnly => {
                cache_fn().await.map_err(ReadError::Cache)
            }

            ReadStrategy::ReadThrough => {
                // Always read from DB
                let result = db_fn().await.map_err(ReadError::Database)?;
                
                // Populate cache on success
                if let (Some(value), Some(populate)) = (&result, populate_fn) {
                    if let Err(e) = populate(value.clone()).await {
                        tracing::warn!(error = %e, "Failed to populate cache");
                    }
                }
                
                Ok(result)
            }
        }
    }

    /// Simple read without cache population.
    pub async fn read_simple<T, CacheFut, DbFut>(
        &self,
        cache_fn: impl FnOnce() -> CacheFut,
        db_fn: impl FnOnce() -> DbFut,
    ) -> Result<Option<T>, ReadError>
    where
        T: Clone + Debug,
        CacheFut: Future<Output = Result<Option<T>, CacheError>>,
        DbFut: Future<Output = Result<Option<T>, DbError>>,
    {
        self.read::<T, _, _, std::future::Ready<Result<(), CacheError>>>(
            cache_fn,
            db_fn,
            None::<fn(T) -> std::future::Ready<Result<(), CacheError>>>,
        )
        .await
    }
}

/// Cache operation error.
#[derive(Debug, thiserror::Error)]
#[error("Cache error: {0}")]
pub struct CacheError(#[from] pub Box<dyn std::error::Error + Send + Sync>);

/// Database operation error.
#[derive(Debug, thiserror::Error)]
#[error("Database error: {0}")]
pub struct DbError(#[from] pub Box<dyn std::error::Error + Send + Sync>);

/// Read operation error.
#[derive(Debug, thiserror::Error)]
pub enum ReadError {
    #[error("Cache error: {0}")]
    Cache(#[from] CacheError),
    #[error("Database error: {0}")]
    Database(#[from] DbError),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_cache_first_hit() {
        let strategy = ReadStrategy::CacheFirst;
        
        let result = strategy
            .read_simple(
                || async { Ok(Some(42)) },
                || async { Ok(Some(99)) },
            )
            .await
            .unwrap();
        
        assert_eq!(result, Some(42)); // Should return cache value
    }

    #[tokio::test]
    async fn test_cache_first_miss() {
        let strategy = ReadStrategy::CacheFirst;
        
        let result = strategy
            .read_simple::<i32, _, _>(
                || async { Ok(None) },
                || async { Ok(Some(99)) },
            )
            .await
            .unwrap();
        
        assert_eq!(result, Some(99)); // Should return DB value
    }

    #[tokio::test]
    async fn test_db_only() {
        let strategy = ReadStrategy::DbOnly;
        
        let result = strategy
            .read_simple(
                || async { Ok(Some(42)) },
                || async { Ok(Some(99)) },
            )
            .await
            .unwrap();
        
        assert_eq!(result, Some(99)); // Should skip cache
    }
}
