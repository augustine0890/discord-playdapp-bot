use chrono::{Datelike, NaiveDate, Utc, Weekday};

use lazy_static::lazy_static;
use rand::Rng;
use serenity::http::Http;
use serenity::Error as SerenityError;
use serenity::{
    model::prelude::*,
    model::{channel::Message, id::UserId},
    prelude::*,
};
use std::collections::HashSet;
use std::sync::Arc;
use tracing::info;

use crate::database::models::LottoGuess;

pub fn is_thu() -> bool {
    let now = Utc::now();
    now.weekday() == chrono::Weekday::Thu
}

pub async fn filter_guilds(ctx: &Context, ready: Ready) {
    let allowed_guilds: Vec<u64> = vec![537515978561683466, 1019782712799805440];
    let guilds = ready.guilds.clone();

    for guild in guilds {
        if !allowed_guilds.contains(&guild.id.0) {
            // If the guild is not in the allowed list, leave the guild
            if let Err(e) = ctx.http.leave_guild(guild.id.0).await {
                info!("Failed to leave guild: {}", e);
            }
        }
    }
}

// Returns the current ISO week number as a tuple of (year, week number)
pub fn get_week_number() -> (i32, u32) {
    let today = Utc::now();
    (today.year(), today.iso_week().week())
}

// Generates a vector of 4 random numbers between 0 and 9
pub fn generate_numbers() -> Vec<i32> {
    let mut rng = rand::thread_rng();
    let numbers: Vec<i32> = (0..4).map(|_| rng.gen_range(0..10)).collect();

    numbers
}

// Returns a NaiveDate object for the Monday of the current week
pub fn get_monday_of_week() -> NaiveDate {
    let today = Utc::now();
    // Calculate the date of Monday of the current week
    let target_date = today
        + chrono::Duration::days(
            Weekday::Mon.num_days_from_sunday() as i64
                - today.weekday().num_days_from_sunday() as i64,
        );

    target_date.date_naive()
}

// Calculate the number of points a user gets in the lotte game.
pub fn calculate_lotto_points(user_numbers: &[i32], draw_numbers: &[i32]) -> (usize, i32) {
    let matches = user_numbers
        .iter()
        .zip(draw_numbers.iter())
        .filter(|(a, b)| a == b)
        .count();
    let points = points_for_matches(matches);
    (matches, points)
}

// Calculate the number of points corresponding to a certain number of matches.
fn points_for_matches(matches: usize) -> i32 {
    match matches {
        1 => 400,
        2 => 1000,
        3 => 5000,
        4 => 100000,
        _ => 0,
    }
}

pub async fn send_dm(
    http: Arc<Http>,
    entry: LottoGuess,
    attend_channel: ChannelId,
) -> Result<Message, SerenityError> {
    // Convert u64 id to UserId
    let user_id = UserId(entry.dc_id);
    // Open a direct message channel with the user
    let dm_channel = user_id.create_dm_channel(&http).await?;

    let content = format!("Congratulations! 👏🏻👏🏻👏🏻 You got {} number(s) correct and you earned {} points in the lotto! 🎰
        \nType “!cp” in <#{}> to check your prize! 🎁", entry.matched_count.unwrap_or(0), entry.points.unwrap_or(0), attend_channel);

    // Send the message and return the resulting Message object
    dm_channel.send_message(&http, |m| m.content(content)).await
}

pub async fn notify_error(http: Arc<Http>, channel_id: ChannelId, mut message: String) {
    // add emoji at the end of the message
    message += " :warning:"; // Add emoji using its alias in markdown format

    // Send the embed message to the channel
    let _ = channel_id
        .send_message(&http, |m| {
            m.embed(|e| {
                e.title("Errors Notification");
                e.description(&message);
                e.color(0xFF0000); // Red color for errors
                e.timestamp(chrono::Utc::now().to_rfc3339())
            })
        })
        .await;
}

lazy_static! {
    pub static ref BAD_EMOJI: HashSet<&'static str> = vec![
        "😠",
        "😤",
        "🤮",
        "💩",
        "🖕🏻",
        "🏻",
        "😾",
        "💢",
        "🇰🇵",
        "👎🏻",
        "👎🏻🏻",
        "😡",
        "👿",
        "🤬",
        "🖕",
        "🖕🏽",
        "👎"
    ]
    .into_iter()
    .collect();
}
