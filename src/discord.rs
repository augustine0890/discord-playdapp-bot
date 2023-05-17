use serenity::{
    async_trait,
    model::gateway::{GatewayIntents, Ready},
    model::id::GuildId,
    prelude::*,
};
use std::env;
use tracing::info;

use crate::commands;
use crate::database::Database;

pub struct Handler {
    pub db: Database,
}

#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, ctx: Context, ready: Ready) {
        info!("{} is connected!", ready.user.name);

        let guild_id = GuildId(
            env::var("DISCORD_GUILD")
                .expect("Expected GUILD_ID in environment")
                .parse()
                .expect("GUILD_ID must be an integer"),
        );

        let _ = guild_id
            .set_application_commands(&ctx.http, |commands| {
                commands.create_application_command(|command| commands::exchange(command))
            })
            .await;
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
