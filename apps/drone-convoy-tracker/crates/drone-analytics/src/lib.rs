//! # Drone Analytics
//!
//! OLAP analytics engine for drone convoy historical analysis.
//! Uses DuckDB for columnar storage and fast analytical queries.
//!
//! ## Features
//!
//! - Historical engagement analysis
//! - Accuracy trends over time
//! - Drone performance comparisons
//! - Mission efficiency metrics
//! - Weapon effectiveness analysis

#![forbid(unsafe_code)]
#![warn(clippy::all, missing_docs)]

pub mod engine;
pub mod error;
pub mod queries;
pub mod reports;

pub use engine::AnalyticsEngine;
pub use error::AnalyticsError;
