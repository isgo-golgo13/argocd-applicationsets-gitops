use tokio::time::{self, Duration};
use tracing::info;
use crate::drones::attack_sortie::sortie_run;

pub async fn run() {
    let mut interval = time::interval(Duration::from_secs(10));
    loop {
        interval.tick().await;
        info!("Starting drone run ...");
        sortie_run().await;
    }
}
