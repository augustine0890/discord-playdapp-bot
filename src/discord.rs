use serenity::{
    async_trait,
    model::gateway::{GatewayIntents, Ready},
    prelude::*,
};
use tracing::info;

use crate::database::Database;

pub struct Handler {
    pub db: Database,
}

#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, _: Context, ready: Ready) {
        info!("{} is connected!", ready.user.name);
    }
}

pub async fn run_discord_bot(token: &str, db: Database) -> tokio::task::JoinHandle<()> {
    let intents = GatewayIntents::GUILD_MESSAGES | GatewayIntents::MESSAGE_CONTENT;
    let mut client = Client::builder(&token, intents)
        .event_handler(Handler { db })
        .await
        .expect("Error creating Discord client");

    let handler = tokio::spawn(async move {
        client.start().await.expect("Error starting Discord client");
    });

    handler
}
