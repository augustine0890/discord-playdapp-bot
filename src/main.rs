use std::sync::Arc;

use tracing::{error, info, Level};
use tracing_subscriber;

use discord_playdapp_bot::config::Config;
use discord_playdapp_bot::database::mongo::MongoDB;
use discord_playdapp_bot::discord::handler::run_discord_bot;
use discord_playdapp_bot::scheduler;

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
    let token = config.discord_token.clone();
    let discord_bot_handle = run_discord_bot(&token, Arc::new(db), Arc::new(config)).await;
    if let Err(why) = discord_bot_handle.await {
        error!("An error occurred while connecting to Discord: {}", why);
    }
}
