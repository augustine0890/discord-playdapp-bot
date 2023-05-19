use tracing::{error, info, Level};
use tracing_subscriber;

use config::Config;
use database::MongoDB;
use discord::run_discord_bot;

mod commands;
mod config;
mod database;
mod discord;
mod scheduler;
mod util;

#[tokio::main]
async fn main() {
    // Setup the tracing
    tracing_subscriber::fmt().with_max_level(Level::INFO).init();

    // Load enviroment variables
    let config = Config::new("config.yaml")
        .await
        .expect("Failed to read configuration file");

    // Connect to the database
    let db = MongoDB::new(&config.mongo_uri)
        .await
        .expect("Failed to connect to database");
    info!("Connected to database");

    // Setup the schedulers
    let scheduler_db = db.clone();
    tokio::spawn(async move {
        scheduler::setup_scheduler(scheduler_db).await;
    });

    // Run the Discord bot
    let discord_bot_handle = run_discord_bot(&config.discord_token, db).await;
    if let Err(why) = discord_bot_handle.await {
        error!("An error occurred while connecting to Discord: {}", why);
    }
}
