use serde::{Serialize, Deserialize};

/// Represents a Drone in the database.
#[derive(Debug, Serialize, Deserialize)]
pub struct Drone {
    pub id: i64,
    pub name: String,
    pub status: String,
    pub last_known_latitude: Option<f64>,
    pub last_known_longitude: Option<f64>,
    pub last_update: Option<String>, // Spanner TIMESTAMP stored as RFC3339 string
}

/// Represents a Waypoint for a Drone.
#[derive(Debug, Serialize, Deserialize)]
pub struct Waypoint {
    pub id: i64,
    pub drone_id: i64,
    pub latitude: f64,
    pub longitude: f64,
    pub timestamp: String, // Spanner TIMESTAMP stored as RFC3339 string
}

/// Represents a Sortie (Mission).
#[derive(Debug, Serialize, Deserialize)]
pub struct Sortie {
    pub id: i64,
    pub drone_id: i64,
    pub status: String,
    pub start_time: String, // Spanner TIMESTAMP stored as RFC3339 string
    pub end_time: Option<String>,
}
