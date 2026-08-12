//! # Drone Simulator
//!
//! Telemetry and engagement simulator for testing the drone convoy tracking system.
//!
//! ## Features
//!
//! - Realistic drone flight path generation
//! - Telemetry data streaming
//! - Randomized engagement simulation
//! - Configurable convoy scenarios

#![forbid(unsafe_code)]
#![warn(clippy::all)]

pub mod convoy;
pub mod engagement;
pub mod flight;
pub mod telemetry;

pub use convoy::ConvoySimulator;
pub use engagement::EngagementSimulator;
pub use flight::FlightPathGenerator;
pub use telemetry::TelemetryGenerator;
