use serenity::{
    async_trait,
    builder::CreateApplicationCommand,
    model::application::command::Command,
    model::application::interaction::Interaction,
    model::channel::Message as DiscordMessage,
    model::gateway::{GatewayIntents, Ready},
    model::id::ChannelId,
    model::prelude::Reaction,
    prelude::*,
};

use std::{collections::HashSet, sync::Arc};
use tracing::{error, info};

use super::slash;
use crate::scheduler::send_daily_report;
use crate::util::filter_guilds;
use crate::{config::EnvConfig, scheduler::lotto_game_scheduler};
use crate::{database::mongo::MongoDB, scheduler::send_announcement_lotto_scheduler};

pub struct Handler {
    pub db: Arc<MongoDB>,
    pub config: Arc<EnvConfig>,
}

#[async_trait]
impl EventHandler for Handler {
    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        match interaction {
            Interaction::ApplicationCommand(command) => match command.data.name.as_str() {
                "exchange" => {
                    if let Err(why) = self.handle_exchange(ctx.clone(), command).await {
                        error!("Error handling exchange: {:?}", why);
                    }
                }
                "lotto" => {
                    if let Err(why) = self.handle_lotto(ctx.clone(), command).await {
                        error!("Error handling lotto: {:?}", why);
                    }
                }
                "lotto-guideline" => {
                    if let Err(why) = self.handle_lotto_guideline(ctx.clone(), command).await {
                        error!("Error handling lotto guideline: {:?}", why);
                    }
                }
                "attendance-guideline" => {
                    if let Err(why) = self.handle_attendance_guideline(ctx.clone(), command).await {
                        error!("Error handling attendance guideline: {:?}", why);
                    }
                }
                _ => info!("Command not found"),
            },
            _ => (),
        }
    }

    async fn ready(&self, ctx: Context, ready: Ready) {
        info!("{} is connected!", ready.user.name);
        // Filter out unwanted guilds, leaving those not in the allowed list
        filter_guilds(&ctx, ready).await;

        // Setup global commands, deleting the "exchange" command if it exists and recreating it
        setup_global_commands(&ctx).await;
    }

    async fn message(&self, ctx: Context, msg: DiscordMessage) {
        if let Err(why) = self.handle_records_command(&msg, &ctx).await {
            error!("Error handling records command: {:?}", why);
        }

        if let Err(why) = self.handle_points_command(&msg, &ctx).await {
            error!("Error handling points command: {:?}", why);
        }
    }

    // When the reaction is added in Discord
    async fn reaction_add(&self, ctx: Context, add_reaction: Reaction) {
        // add activity points based on the reaction poll.
        if let Err(why) = self.poll_reaction(&ctx, &add_reaction).await {
            error!("Error adding polling reaction: {:?}", why);
        }

        // add activity points based on the reaction type.
        if let Err(why) = self.reaction_activity(&ctx, &add_reaction).await {
            error!("Error adding reacting activity reaction: {:?}", why);
        }
    }
}

pub async fn run_discord_bot(
    token: &str,
    db: Arc<MongoDB>,       // Make sure to pass an Arc<MongoDB> instead of &MongoDB
    config: Arc<EnvConfig>, // Same with the EnvConfig
) -> tokio::task::JoinHandle<()> {
    // Define the necessary gateway intents
    let intents = GatewayIntents::GUILD_MESSAGES
        | GatewayIntents::MESSAGE_CONTENT
        | GatewayIntents::GUILDS
        | GatewayIntents::GUILD_INTEGRATIONS
        | GatewayIntents::GUILD_MESSAGES
        | GatewayIntents::GUILD_MESSAGE_REACTIONS
        | GatewayIntents::DIRECT_MESSAGE_REACTIONS;
    // Build the Discord client with the token, intents and event handler
    let client = Client::builder(&token, intents)
        .event_handler(Handler {
            db: Arc::clone(&db),
            config: Arc::clone(&config),
        })
        .await
        .expect("Error creating Discord client");

    // Clone the HTTP context for use in the daily report task
    let http = client.cache_and_http.http.clone();
    // Create a shared, mutable reference to the client using an Arc<Mutex<>>
    let shared_client = Arc::new(Mutex::new(client));

    // Spawn a new async task to handle running the Discord bot
    let handler = tokio::spawn(async move {
        // The channel ID to send the daily reports
        let channel_id = ChannelId(1054296641651347486); // Replace with the specific channel ID
                                                         // Start the daily report in a new async task
        send_daily_report(http.clone(), channel_id).await;

        lotto_game_scheduler(Arc::clone(&db), Arc::clone(&config), http.clone()).await;

        send_announcement_lotto_scheduler(Arc::clone(&db), Arc::clone(&config), http.clone()).await;

        // Lock the shared client for use in this task
        let mut locked_client = shared_client.lock().await;
        // Start the Discord client and handle any errors
        locked_client
            .start()
            .await
            .expect("Error starting Discord client");
    });

    handler
}

pub async fn setup_global_commands(ctx: &Context) {
    // Fetch existing global commands.
    let global_commands = Command::get_global_application_commands(&ctx.http)
        .await
        .unwrap();

    let commands_to_delete = [
        "exchange",
        "lotto",
        "lotto-guideline",
        "attendance-guideline",
    ];
    let commands_to_delete: HashSet<&str> = commands_to_delete.iter().cloned().collect();

    // Loop over the global commands and delete the command named "exchange" if it exists.
    for command in global_commands {
        if commands_to_delete.contains(command.name.as_str()) {
            Command::delete_global_application_command(&ctx.http, command.id)
                .await
                .expect("Failed to delete global command");
        }
    }

    let command_setups: Vec<fn(&mut CreateApplicationCommand) -> &mut CreateApplicationCommand> = vec![
        slash::exchange,
        slash::lotto,
        slash::lotto_guideline,
        slash::attendance_guideline,
    ];

    for setup in command_setups {
        let _ =
            Command::create_global_application_command(&ctx.http, |command| setup(command)).await;
    }
}
