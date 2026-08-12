//! Analytics error types.

use thiserror::Error;

/// Analytics errors.
#[derive(Error, Debug)]
pub enum AnalyticsError {
    /// DuckDB error
    #[error("DuckDB error: {0}")]
    DuckDb(#[from] duckdb::Error),

    /// Query execution error
    #[error("Query error: {0}")]
    Query(String),

    /// Data conversion error
    #[error("Data conversion error: {0}")]
    Conversion(String),

    /// Invalid parameter
    #[error("Invalid parameter: {0}")]
    InvalidParameter(String),

    /// No data found
    #[error("No data found for query")]
    NoData,

    /// IO error
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// Result type for analytics operations.
pub type Result<T> = std::result::Result<T, AnalyticsError>;
