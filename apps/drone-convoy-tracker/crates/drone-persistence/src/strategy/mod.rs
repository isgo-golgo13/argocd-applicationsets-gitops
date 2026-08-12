//! # Strategy Module
//!
//! Enum-based cache/database access strategies using dispatch pattern.
//!
//! ## Available Strategies
//!
//! ### Read Strategies
//! - `CacheFirst` - Check cache, fall back to DB on miss (default)
//! - `DbOnly` - Skip cache entirely
//! - `CacheOnly` - Never hit database
//! - `ReadThrough` - Always read DB, populate cache
//!
//! ### Write Strategies  
//! - `WriteThrough` - Write to both cache and DB synchronously (default)
//! - `WriteAround` - Write DB only, invalidate cache
//! - `WriteBack` - Write cache first, async DB write
//! - `DbOnly` - Write DB only, no cache interaction
//!
//! ## Example
//!
//! ```rust,ignore
//! use drone_persistence::strategy::{ReadStrategy, WriteStrategy};
//!
//! let read = ReadStrategy::CacheFirst;
//! let write = WriteStrategy::WriteThrough;
//!
//! // Use with repository
//! let result = read.read_simple(
//!     || cache.get(key),
//!     || db.query(key),
//! ).await?;
//! ```

pub mod read_strategy;
pub mod write_strategy;

pub use read_strategy::{CacheError, DbError, ReadError, ReadStrategy};
pub use write_strategy::{WriteError, WriteStrategy};
