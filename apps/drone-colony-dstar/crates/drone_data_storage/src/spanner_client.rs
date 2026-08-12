use google_cloud_spanner::client::{Client, ClientConfig};
use google_cloud_spanner::SessionConfig;
use tracing::{info, error};

pub async fn init_spanner_schema() {
    let client = Client::new(ClientConfig {
        instance_id: "spanner-instance".to_string(),
        database_id: "drone_tracking".to_string(),
        session_config: SessionConfig::default(),
    }).await.unwrap();

    let create_tables_sql = include_str!("schema.sql"); // Load schema from file
    match client.execute_sql(create_tables_sql, &[]).await {
        Ok(_) => info!("Spanner schema loaded successfully"),
        Err(e) => error!("⚠️ Failed to load Spanner schema: {:?}", e),
    }
}

pub async fn update_drone_status(drone_id: i64, latitude: f64, longitude: f64) {
    let client = Client::new(ClientConfig {
        instance_id: "spanner-instance".to_string(),
        database_id: "drone_tracking".to_string(),
        session_config: SessionConfig::default(),
    }).await.unwrap();

    let sql = "UPDATE drones SET last_known_latitude = @lat, last_known_longitude = @long, last_update = CURRENT_TIMESTAMP() WHERE id = @id";
    let params = [("lat", latitude.into()), ("long", longitude.into()), ("id", drone_id.into())];

    match client.execute_sql(sql, &params).await {
        Ok(_) => info!("Updated Drone {} position in Spanner.", drone_id),
        Err(e) => error!("⚠️ Failed to update drone position: {:?}", e),
    }
}
