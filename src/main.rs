use tracing::{error, info, Level};
use tracing_subscriber;

use config::Config;
use database::Database;
use discord::run_discord_bot;

mod config;
mod database;
mod discord;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt().with_max_level(Level::INFO).init();

    let config = Config::new("config.yaml").expect("Failed to read configuration file");
    let db = Database::new(&config.mongo_uri)
        .await
        .expect("Failed to connect to database");
    info!("Connected to database");

    // Run the Discord bot
    let discord_bot_handle = run_discord_bot(&config.discord_token, db).await;
    if let Err(why) = discord_bot_handle.await {
        error!("An error occurred while connecting to Discord: {}", why);
    }
}
