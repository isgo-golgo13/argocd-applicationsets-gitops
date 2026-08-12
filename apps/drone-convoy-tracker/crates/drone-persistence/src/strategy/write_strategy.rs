//! Write strategy implementations using enum dispatch.

use std::fmt::Debug;
use std::future::Future;

use super::read_strategy::{CacheError, DbError};

/// Write strategy enum - determines cache/db write pattern.
#[derive(Debug, Clone, Copy, Default)]
pub enum WriteStrategy {
    /// Write to both cache and DB synchronously
    #[default]
    WriteThrough,
    /// Write to DB only, invalidate cache
    WriteAround,
    /// Write to cache immediately, async write to DB
    WriteBack,
    /// Write to DB only, no cache interaction
    DbOnly,
}

impl WriteStrategy {
    /// Execute a write operation according to the strategy.
    ///
    /// - `cache_fn`: Async function to write to cache
    /// - `db_fn`: Async function to write to database
    /// - `invalidate_fn`: Optional async function to invalidate cache
    pub async fn write<T, CacheFut, DbFut, InvalidateFut>(
        &self,
        value: &T,
        cache_fn: impl FnOnce(&T) -> CacheFut,
        db_fn: impl FnOnce(&T) -> DbFut,
        invalidate_fn: Option<impl FnOnce() -> InvalidateFut>,
    ) -> Result<(), WriteError>
    where
        T: Debug,
        CacheFut: Future<Output = Result<(), CacheError>>,
        DbFut: Future<Output = Result<(), DbError>>,
        InvalidateFut: Future<Output = Result<(), CacheError>>,
    {
        match self {
            WriteStrategy::WriteThrough => {
                // Write to DB first
                db_fn(value).await.map_err(WriteError::Database)?;
                
                // Then write to cache
                if let Err(e) = cache_fn(value).await {
                    tracing::warn!(error = %e, "Failed to write to cache");
                }
                
                Ok(())
            }

            WriteStrategy::WriteAround => {
                // Write to DB only
                db_fn(value).await.map_err(WriteError::Database)?;
                
                // Invalidate cache
                if let Some(invalidate) = invalidate_fn {
                    if let Err(e) = invalidate().await {
                        tracing::warn!(error = %e, "Failed to invalidate cache");
                    }
                }
                
                Ok(())
            }

            WriteStrategy::WriteBack => {
                // Write to cache immediately
                cache_fn(value).await.map_err(WriteError::Cache)?;
                
                // Queue async write to DB (simplified - just write now)
                // In production, this would use a background task queue
                if let Err(e) = db_fn(value).await {
                    tracing::error!(error = %e, "Failed to write to DB (write-back)");
                    // Don't fail - cache has the data
                }
                
                Ok(())
            }

            WriteStrategy::DbOnly => {
                db_fn(value).await.map_err(WriteError::Database)
            }
        }
    }

    /// Simple write without invalidation.
    pub async fn write_simple<T, CacheFut, DbFut>(
        &self,
        value: &T,
        cache_fn: impl FnOnce(&T) -> CacheFut,
        db_fn: impl FnOnce(&T) -> DbFut,
    ) -> Result<(), WriteError>
    where
        T: Debug,
        CacheFut: Future<Output = Result<(), CacheError>>,
        DbFut: Future<Output = Result<(), DbError>>,
    {
        self.write::<T, _, _, std::future::Ready<Result<(), CacheError>>>(
            value,
            cache_fn,
            db_fn,
            None::<fn() -> std::future::Ready<Result<(), CacheError>>>,
        )
        .await
    }
}

/// Write operation error.
#[derive(Debug, thiserror::Error)]
pub enum WriteError {
    #[error("Cache error: {0}")]
    Cache(#[from] CacheError),
    #[error("Database error: {0}")]
    Database(#[from] DbError),
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    #[tokio::test]
    async fn test_write_through() {
        let strategy = WriteStrategy::WriteThrough;
        let cache_called = Arc::new(AtomicBool::new(false));
        let db_called = Arc::new(AtomicBool::new(false));
        
        let cache_flag = cache_called.clone();
        let db_flag = db_called.clone();
        
        strategy
            .write_simple(
                &42,
                |_| {
                    cache_flag.store(true, Ordering::SeqCst);
                    async { Ok(()) }
                },
                |_| {
                    db_flag.store(true, Ordering::SeqCst);
                    async { Ok(()) }
                },
            )
            .await
            .unwrap();
        
        assert!(cache_called.load(Ordering::SeqCst));
        assert!(db_called.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn test_db_only() {
        let strategy = WriteStrategy::DbOnly;
        let cache_called = Arc::new(AtomicBool::new(false));
        let db_called = Arc::new(AtomicBool::new(false));
        
        let cache_flag = cache_called.clone();
        let db_flag = db_called.clone();
        
        strategy
            .write_simple(
                &42,
                |_| {
                    cache_flag.store(true, Ordering::SeqCst);
                    async { Ok(()) }
                },
                |_| {
                    db_flag.store(true, Ordering::SeqCst);
                    async { Ok(()) }
                },
            )
            .await
            .unwrap();
        
        assert!(!cache_called.load(Ordering::SeqCst)); // Cache NOT called
        assert!(db_called.load(Ordering::SeqCst));
    }
}
