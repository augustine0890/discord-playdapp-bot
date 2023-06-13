use chrono::{Datelike, Utc};
use lazy_static::lazy_static;
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
