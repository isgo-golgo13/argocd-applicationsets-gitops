use crate::data_storage::spanner_client::update_drone_status;
use tracing::info;

pub async fn sortie_run() {
    let drones = vec![("drone-1", 34.0522, -118.2437)];

    for (id, lat, long) in drones {
        info!("Drone {} reached waypoint ({}, {})", id, lat, long);
        update_drone_status(id, lat, long).await;
    }
}
