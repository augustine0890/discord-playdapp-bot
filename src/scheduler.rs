use crate::database::mongo::MongoDB;
use chrono::{NaiveDate, Utc};
use cron::Schedule;
use serenity::{http::Http, model::id::ChannelId};
use std::{str::FromStr, sync::Arc};
use tracing::{error, info};

pub async fn setup_scheduler(database: MongoDB) {
    // let thursday_schedule = Schedule::from_str("0 */5 * * * *").unwrap(); // every 5 mins
    let thursday_schedule = Schedule::from_str("0 1 0 * * 5").unwrap();
    let friday_schedule = Schedule::from_str("0 1 0 * * 6").unwrap();
    // let friday_schedule = Schedule::from_str("0 */10 * * * *").unwrap(); // every 10 mins

    let database_thursday = database.clone();

    // Spawn a new task for the Thursday schedule
    tokio::spawn(async move {
        let mut now = Utc::now();
        loop {
            // Compute the time until the next Thursday event.
            if let Some(next_thursday) = thursday_schedule.upcoming(chrono::Utc).next() {
                if next_thursday > now {
                    let duration = (next_thursday - now).to_std().unwrap();
                    let duration_in_days = (duration.as_secs() as f64 / 86400.0).round();
                    info!(
                        "[Sending Tickets] Waiting [{} days] until next scheduled event: [{}]",
                        duration_in_days, next_thursday
                    );
                    tokio::time::sleep(duration).await;

                    // Try to run task here until it succeeds.
                    let mut task_succeeded = false;

                    // Run task here.
                    while !task_succeeded {
                        match database_thursday.update_all_submitted_to_processing().await {
                            Ok(_) => {
                                info!("Changed successfully to processing records");
                                task_succeeded = true;
                            }
                            Err(e) => {
                                error!("Failed to updated processing records: {}", e);
                                // Sleep for 5 minutes before retrying.
                                tokio::time::sleep(tokio::time::Duration::from_secs(300)).await;
                            }
                        }
                    }

                    // Update "now" for the next iteration.
                    now = Utc::now();
                }
            }
        }
    });

    let database_friday = database.clone();
    // Spawn a new task for the Friday schedule
    tokio::spawn(async move {
        let mut now = Utc::now();
        loop {
            if let Some(next_friday) = friday_schedule.upcoming(chrono::Utc).next() {
                if next_friday > now {
                    let duration = (next_friday - now).to_std().unwrap();
                    let duration_in_days = (duration.as_secs() as f64 / 86400.0).round();
                    info!(
                        "[Sent Tickets] Waiting [{} days] until next scheduled event: [{}]",
                        duration_in_days, next_friday
                    );
                    tokio::time::sleep(duration).await;

                    let mut task_succeeded = false;

                    while !task_succeeded {
                        match database_friday.update_all_processing_to_completed().await {
                            Ok(_) => {
                                info!("Changed successfully to completed records");
                                task_succeeded = true;
                            }
                            Err(e) => {
                                error!("Failed to updated completed records: {}", e);
                                // Sleep for 5 minutes.
                                tokio::time::sleep(tokio::time::Duration::from_secs(300)).await;
                            }
                        }
                    }

                    // Update "now" for the next iteration.
                    now = Utc::now();
                }
            }
        }
    });

    let database_monthly = database.clone();
    // The first day of every month
    let monthly = Schedule::from_str("0 0 0 1 * *").unwrap();

    tokio::spawn(async move {
        let mut now = Utc::now();
        loop {
            // let mut upcoming_times = monthly.upcoming(chrono::Utc);
            if let Some(next_month) = monthly.upcoming(chrono::Utc).next() {
                if next_month > now {
                    let duration = (next_month - now).to_std().unwrap();
                    let duration_in_days = (duration.as_secs() as f64 / 86400.0).round();
                    info!(
                        "[Remove Data] Waiting [{} days] until next scheduled event: [{}]",
                        duration_in_days, next_month
                    );

                    tokio::time::sleep(duration).await;

                    let mut task_succeeded = false;

                    while !task_succeeded {
                        match database_monthly.clean_documents().await {
                            Ok(result) => {
                                let deleted_count = result.deleted_count;
                                info!("Deleted {} documents", deleted_count);
                                task_succeeded = true
                            }
                            Err(e) => {
                                error!("Error deleting documents {}", e);
                                tokio::time::sleep(tokio::time::Duration::from_secs(300)).await
                            }
                        }
                    }
                    now = Utc::now();
                }
            }
        }
    });
    // Clone the database instance for use in the new task
    let db_handle = database.clone();
    // The schedule string represents "at 00:00:00 on every Monday"
    let weekly_schedule = Schedule::from_str("0 0 0 * * 2").unwrap();
    // let weekly_schedule = Schedule::from_str("0 */2 * * * *").unwrap();

    tokio::spawn(async move {
        let mut current_time = Utc::now();
        loop {
            // Get the next scheduled time according to the weekly schedule
            if let Some(next_scheduled_time) = weekly_schedule.upcoming(chrono::Utc).next() {
                // If the next scheduled time is in the future...
                if next_scheduled_time > current_time {
                    // Calculate the amount of time until the next scheduled event
                    let sleep_duration = (next_scheduled_time - current_time).to_std().unwrap();
                    let sleep_days = (sleep_duration.as_secs() as f64 / 86400.0).round();
                    info!(
                        "[Draw Generation] Waiting [{} days] until next scheduled event: [{}]",
                        sleep_days, next_scheduled_time
                    );

                    // Sleep until the next scheduled event
                    tokio::time::sleep(sleep_duration).await;

                    let mut task_succeeded = false;

                    // Keep trying to generate the weekly draw until successful
                    while !task_succeeded {
                        match db_handle.add_weekly_draw().await {
                            Ok(_) => {
                                info!("Successfully generated draw numbers");
                                task_succeeded = true
                            }
                            Err(e) => {
                                error!("Error generating draw numbers: {}", e);
                                // If there was an error, wait for a minute before retrying
                                tokio::time::sleep(tokio::time::Duration::from_secs(60)).await
                            }
                        }
                    }
                    // Update the current time
                    current_time = Utc::now();
                }
            }
        }
    });
}

pub async fn send_daily_report(http: Arc<Http>, channel_id: ChannelId) {
    // The specific date (January 31, 2023)
    let started_day = NaiveDate::from_ymd_opt(2023, 1, 31);
    // At 9:00 AM (UTC + 9) every day
    let daily = Schedule::from_str("0 0 0 * * *").unwrap();
    // let daily = Schedule::from_str("0 * * * * *").unwrap();

    // Run concurrently (asynchronous task)
    tokio::spawn(async move {
        let mut now = Utc::now();
        loop {
            // Next time event
            if let Some(next_daily) = daily.upcoming(chrono::Utc).next() {
                // The next time event is in the future
                if next_daily > now {
                    let duration = (next_daily - now).to_std().unwrap();
                    info!("[Daily Report] The next scheduled event: [{}]", next_daily);
                    // Pause the task for the duration until the next scheduled event
                    tokio::time::sleep(duration).await;
                    now = Utc::now();
                    // Calculate the duratioin from the started day until now
                    let available_days =
                        now.date_naive().signed_duration_since(started_day.unwrap());
                    let days = available_days.num_days();

                    let message = format!(
                        "🗣️ The Points System has run for **{}** days without crashing.🥳",
                        days
                    );
                    // Send the embed message to the channel
                    let _ = channel_id
                        .send_message(&http, |m| {
                            m.embed(|e| {
                                e.title("Application Uptime");
                                e.description(&message);
                                e.color(0x00ff00);
                                e.timestamp(chrono::Utc::now().to_rfc3339())
                            })
                        })
                        .await;
                }
            }
        }
    });
}
