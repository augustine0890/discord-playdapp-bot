use chrono::{Datelike, NaiveDate, Utc, Weekday};

use lazy_static::lazy_static;
use rand::Rng;
use serenity::{model::prelude::*, prelude::*};
use std::collections::HashSet;
use tracing::info;

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
fn calculate_lotto_points(user_numbers: &[i32], draw_number: &[i32]) -> i32 {
    let matches = user_numbers
        .iter()
        .zip(draw_number.iter())
        .filter(|(a, b)| a == b)
        .count();
    points_for_matches(matches)
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
