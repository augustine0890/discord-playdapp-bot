use crate::{
    config::EnvConfig,
    database::mongo::MongoDB,
    util::{get_week_number, notify_error, send_dm},
};
use chrono::{NaiveDate, Utc};
use cron::Schedule;
use serenity::{http::Http, model::id::ChannelId};
use std::{collections::HashMap, error::Error, str::FromStr, sync::Arc};
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
    // let weekly_schedule = Schedule::from_str("0 */1 * * * *").unwrap();

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

pub async fn lotto_game_scheduler(database: Arc<MongoDB>, config: Arc<EnvConfig>, http: Arc<Http>) {
    // The schedule string represents "at 02:58:00 on every Monday"
    // let weekly_schedule = Schedule::from_str("0 */1 * * * *").unwrap();
    let weekly_schedule = Schedule::from_str("0 58 2 * * 2").unwrap();

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
                        "[Lotto Game DM] Waiting for [{} days] until next scheduled event: [{}]",
                        sleep_days, next_scheduled_time
                    );

                    // Sleep until the next scheduled event
                    tokio::time::sleep(sleep_duration).await;

                    let mut task_succeeded = false;

                    // Keep trying to process the last week entries until successful
                    while !task_succeeded {
                        match process_last_week_lotto_guesses(&*config, &*database, http.clone())
                            .await
                        {
                            Ok(_) => {
                                info!("[Lotto Game DM] Successfully processed last week entries");
                                task_succeeded = true
                            }
                            Err(e) => {
                                error!("[Lotto Game DM] Error processing last week entries: {}", e);
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

pub async fn process_last_week_lotto_guesses(
    config: &EnvConfig,
    database: &MongoDB,
    http: Arc<Http>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let (mut year, current_week) = get_week_number();
    let last_week;
    if current_week == 1 {
        last_week = 52;
        year -= 1; // The last week was in the previous year
    } else {
        last_week = current_week - 1;
    }

    // Fetch all entries from the last week with is_any_matched set to true
    let last_week_entries = database
        .get_lotto_guesses(year, last_week, Some(false))
        .await?;

    // If last_week_entries is empty, return and do nothing
    if last_week_entries.is_empty() {
        return Ok(());
    }

    let attendance_channel_id = config.attendance_channel;
    let attendance_channel = ChannelId(attendance_channel_id);

    // Iterate over all the matching entries
    for entry in last_week_entries {
        // Send DM to the user based on dc_id
        send_dm(http.clone(), entry.clone(), attendance_channel).await?;
        // Update the dm_sent flag to true for this entry
        database.update_dm_sent_flag(entry.id.unwrap()).await?;

        database
            .adjust_user_points(&entry.dc_id.to_string(), entry.points.unwrap())
            .await?;
    }

    Ok(())
}

pub async fn send_announcement_lotto_results(
    config: &EnvConfig,
    database: &MongoDB,
    http: Arc<Http>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let (mut year, current_week) = get_week_number();
    let last_week;
    if current_week == 1 {
        last_week = 52;
        year -= 1; // The last week was in the previous year
    } else {
        last_week = current_week - 1;
    }

    let winning_numbers = match database.get_lotto_draw(year, last_week).await {
        Ok(numbers) => numbers,
        Err(e) => {
            error!("Error fetching lotto draw numbers: {}", e);
            return Err(Box::new(e));
        }
    };

    let winning_numbers_string = winning_numbers
        .iter()
        .map(|n| n.to_string())
        .collect::<Vec<String>>()
        .join(", ");

    let lotto_guesses = match database.get_lotto_guesses(year, last_week, None).await {
        Ok(guesses) => guesses,
        Err(e) => {
            error!("Error fetching lotto guesses: {}", e);
            return Err(Box::new(e));
        }
    };

    let mut winners_count: HashMap<i32, i32> = HashMap::new();

    for guess in lotto_guesses {
        let match_count = guess.matched_count.unwrap_or(0);
        *winners_count.entry(match_count).or_insert(0) += 1;
    }

    let mut message = format!(
        "**Weekly Lotto Results - Week {} 🎰**\n\n\
        Hello @everyone! We’re thrilled to announce the results of last week’s lotto! 😆Thank you all for your participation and patience!! The anticipation has been building, and now it's time to reveal the winning numbers! Let's get started🔥 \n\n\
        **Winning Lotto Numbers: {}**\n\n\
        **Number of Winners:**\n",
        last_week,
        winning_numbers_string,
    );

    message += &format!(
        "4️⃣ matching numbers: {}\n",
        winners_count.get(&4).unwrap_or(&0)
    );
    message += &format!(
        "3️⃣ matching numbers: {}\n",
        winners_count.get(&3).unwrap_or(&0)
    );
    message += &format!(
        "2️⃣ matching numbers: {}\n",
        winners_count.get(&2).unwrap_or(&0)
    );
    message += &format!(
        "1️⃣ matching number: {}\n",
        winners_count.get(&1).unwrap_or(&0)
    );

    let attendance_channel_id = config.attendance_channel;
    let attendance_channel = ChannelId(attendance_channel_id);

    message.push_str(&format!("\nWinners, please read the DM 📨 that we sent you and check your prize in <#{}>!🎁 \n\n\
    Thank you once again to everyone who participated in last week's lotto!🧡 🫶🏻 \n\
    The entry period will open every Monday 00:00 (UTC+0), get ready for another exciting round of the lotto this week!\n\n\
    **Good luck to you all!**🍀", attendance_channel));

    let lotto_channel_id = config.lotto_channel;
    let lotto_channel = ChannelId(lotto_channel_id);

    // send the message
    let _ = lotto_channel.say(http, message).await;

    Ok(())
}

pub async fn send_announcement_lotto_scheduler(
    database: Arc<MongoDB>,
    config: Arc<EnvConfig>,
    http: Arc<Http>,
    channel_id: ChannelId,
) {
    // The schedule string represents "at 03:00:00 on every Monday"
    let weekly_schedule = Schedule::from_str("0 0 3 * * 2").unwrap();
    // let weekly_schedule = Schedule::from_str("0 */1 * * * *").unwrap();

    tokio::spawn(async move {
        let mut current_time = Utc::now();
        loop {
            if let Some(next_scheduled_time) = weekly_schedule.upcoming(chrono::Utc).next() {
                if next_scheduled_time > current_time {
                    let sleep_duration = (next_scheduled_time - current_time).to_std().unwrap();
                    let sleep_days = (sleep_duration.as_secs() as f64 / 86400.0).round();
                    info!(
                        "[Lotto Results Announcement] Waiting for [{} days] until next scheduled event: [{}]",
                        sleep_days, next_scheduled_time
                    );

                    tokio::time::sleep(sleep_duration).await;

                    let mut task_succeeded = false;
                    let mut retries = 0;

                    while !task_succeeded && retries < 3 {
                        // Limit retries to 3 times
                        match send_announcement_lotto_results(&*config, &*database, http.clone())
                            .await
                        {
                            Ok(_) => {
                                info!("[Lotto Results Announcement] Successfully sending lotto results");
                                task_succeeded = true
                            }
                            Err(e) => {
                                retries += 1;
                                error!(
                                    "[Lotto Results Announcement] Error sending lotto results: {}. Retry attempt: {}",
                                    e, retries
                                );
                                notify_error(http.clone(), channel_id, e.to_string()).await;

                                tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
                            }
                        }
                    }

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
