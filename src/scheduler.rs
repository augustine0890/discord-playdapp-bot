use crate::database::MongoDB;
use chrono::Utc;
use cron::Schedule;
use std::str::FromStr;
use tracing::{error, info};

pub async fn setup_scheduler(database: MongoDB) {
    // let thursday_schedule = Schedule::from_str("0 */5 * * * *").unwrap(); // every 2 mins
    let thursday_schedule = Schedule::from_str("* * * * * 5").unwrap();
    let friday_schedule = Schedule::from_str("* * * * * 6").unwrap();
    // let friday_schedule = Schedule::from_str("0 */10 * * * *").unwrap(); // every 10 mins

    let database_clone = database.clone();

    // Spawn a new task for the Thursday schedule
    tokio::spawn(async move {
        let mut now = Utc::now();
        loop {
            // Compute the time until the next Thursday event.
            if let Some(next_thursday) = thursday_schedule.upcoming(chrono::Utc).next() {
                if next_thursday > now {
                    let duration = (next_thursday - now).to_std().unwrap();
                    info!(
                        "[Sending Tickets] Waiting until next scheduled event: [{}]",
                        next_thursday
                    );
                    tokio::time::sleep(duration).await;
                    // Run task here.
                    match database_clone.update_all_submitted_to_processing().await {
                        Ok(_) => info!("Changed successfully to processing records"),
                        Err(e) => error!("Failed to updated processing records: {}", e),
                    }
                    // Update "now" for the next iteration.
                    now = Utc::now();
                }
            }
        }
    });

    // Spawn a new task for the Friday schedule
    tokio::spawn(async move {
        let mut now = Utc::now();
        loop {
            if let Some(next_friday) = friday_schedule.upcoming(chrono::Utc).next() {
                if next_friday > now {
                    let duration = (next_friday - now).to_std().unwrap();
                    info!(
                        "[Sent Tickets] Waiting until next scheduled event: [{}]",
                        next_friday
                    );
                    tokio::time::sleep(duration).await;
                    match database.update_all_processing_to_completed().await {
                        Ok(_) => info!("Changed successfully to completed records"),
                        Err(e) => error!("Failed to updated completed records: {}", e),
                    }
                    now = Utc::now();
                }
            }
        }
    });
}
