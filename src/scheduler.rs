use crate::database::MongoDB;
use chrono::Utc;
use cron::Schedule;
use std::str::FromStr;
use tracing::{error, info};

pub async fn setup_scheduler(database: MongoDB) {
    let schedule = Schedule::from_str("* * * * * 5").unwrap();
    // let schedule = Schedule::from_str("0 * * * * *").unwrap(); // every min
    let mut now = Utc::now();

    loop {
        // Compute the time until the next event.
        if let Some(next) = schedule.upcoming(chrono::Utc).next() {
            if next > now {
                let duration = (next - now).to_std().unwrap();
                info!("Waiting until next scheduled event [{}]", next);
                tokio::time::sleep(duration).await;
            }
        }

        // Run your task here.
        match database.update_all_submitted_to_processing().await {
            Ok(_) => info!("Records updated successfully"),
            Err(e) => error!("Failed to updated records: {}", e),
        }

        // Update "now" for the next iteration.
        now = Utc::now();
    }
}
