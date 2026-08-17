//! # Tactical Theaters (frontend view)
//!
//! Re-exports the shared theater table from `drone-domain`. The simulator
//! flies these exact routes and posts real positions; the map draws these
//! exact routes as pins. ONE array, two consumers — that is what makes the
//! GPS readout on the drone cards honest in every theater.

pub use drone_domain::theaters::{Theater, TheaterId};
