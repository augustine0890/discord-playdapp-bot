use tracing::{error, info, Level};
use tracing_subscriber;

use config::Config;
use database::Database;
use discord::run_discord_bot;

mod commands;
mod config;
mod database;
mod discord;

#[tokio::main]
async fn main() {
    // Setup the tracing
    tracing_subscriber::fmt().with_max_level(Level::INFO).init();

    // Load enviroment variables
    let config = Config::new("config.yaml")
        .await
        .expect("Failed to read configuration file");

    // Connect to the database
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
