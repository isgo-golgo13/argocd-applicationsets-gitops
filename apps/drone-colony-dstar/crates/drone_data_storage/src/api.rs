use crate::data_storage::spanner_client;
use crate::data_storage::schema::{Drone, Waypoint, Sortie};
use tracing::{info, error};

/// Fetches all active drones from Spanner.
pub async fn get_active_drones() -> Vec<Drone> {
    match spanner_client::get_active_drones().await {
        Ok(drones) => {
            info!("Retrieved {} active drones", drones.len());
            drones
        }
        Err(e) => {
            error!("Failed to fetch active drones: {:?}", e);
            vec![]
        }
    }
}

/// Fetches a drone's latest position by its ID.
pub async fn get_drone_position(drone_id: i64) -> Option<(f64, f64)> {
    match spanner_client::get_drone_by_id(drone_id).await {
        Ok(Some(drone)) => {
            info!("Drone {} position: ({:?}, {:?})", drone_id, drone.last_known_latitude, drone.last_known_longitude);
            Some((drone.last_known_latitude.unwrap_or(0.0), drone.last_known_longitude.unwrap_or(0.0)))
        }
        Ok(None) => {
            error!("Drone {} not found.", drone_id);
            None
        }
        Err(e) => {
            error!("Failed to fetch drone {} position: {:?}", drone_id, e);
            None
        }
    }
}

/// Records a drone reaching a waypoint.
pub async fn record_waypoint(drone_id: i64, latitude: f64, longitude: f64) {
    match spanner_client::insert_waypoint(drone_id, latitude, longitude).await {
        Ok(_) => info!("Waypoint recorded for drone {}", drone_id),
        Err(e) => error!("Failed to record waypoint for drone {}: {:?}", drone_id, e),
    }
}

/// Starts a new sortie (mission) for a drone.
pub async fn start_sortie(drone_id: i64) -> Option<i64> {
    match spanner_client::start_sortie(drone_id).await {
        Ok(sortie_id) => {
            info!("Started sortie {} for drone {}", sortie_id, drone_id);
            Some(sortie_id)
        }
        Err(e) => {
            error!("Failed to start sortie for drone {}: {:?}", drone_id, e);
            None
        }
    }
}

/// Marks a sortie as completed or failed.
pub async fn complete_sortie(sortie_id: i64, success: bool) {
    let status = if success { "COMPLETED" } else { "FAILED" };
    match spanner_client::update_sortie_status(sortie_id, status).await {
        Ok(_) => info!("Sortie {} marked as {}", sortie_id, status),
        Err(e) => error!("⚠️ Failed to update sortie {}: {:?}", sortie_id, e),
    }
}
