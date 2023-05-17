use ethers::types::Address;
use ethers::utils::to_checksum;
use serenity::model::application::command::Command;
use serenity::model::prelude::interaction::MessageFlags;
use serenity::{
    async_trait,
    model::application::interaction::application_command::ApplicationCommandInteraction,
    model::application::interaction::{Interaction, InteractionResponseType},
    model::gateway::{GatewayIntents, Ready},
    prelude::*,
};

use std::env;
use std::str::FromStr;
use tracing::{error, info};

use crate::commands;
use crate::database::Database;

pub struct Handler {
    pub db: Database,
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
                _ => info!("Command not found"),
            },
            _ => (),
        }
    }

    async fn ready(&self, ctx: Context, ready: Ready) {
        info!("{} is connected!", ready.user.name);

        // Fetch existing global commands.
        let global_commands = Command::get_global_application_commands(&ctx.http)
            .await
            .unwrap();
        // Loop over the global commands and delete the command named "exchange" if it exists.
        for command in global_commands {
            if command.name == "exchange" {
                Command::delete_global_application_command(&ctx.http, command.id)
                    .await
                    .expect("Failed to delete global command");
            }
        }
        let _ = Command::create_global_application_command(&ctx.http, |command| {
            commands::exchange(command)
        })
        .await;
    }
}

impl Handler {
    async fn handle_exchange(
        &self,
        ctx: Context,
        command: ApplicationCommandInteraction,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Check for the correct channel
        let attendance_channel: u64 = env::var("ATTENDANCE_CHANNEL")?
            .parse()
            .expect("ATTENDANCE_CHANNEL must be an integer");
        if command.channel_id.as_u64() != &attendance_channel {
            let _ = command
                .create_interaction_response(&ctx.http, |r| {
                    r.kind(InteractionResponseType::ChannelMessageWithSource)
                        .interaction_response_data(|m| {
                            m.content(format!(
                                "Please go to the <#{}> channel to exchange Items.",
                                attendance_channel
                            ))
                            // .allowed_mentions(|am| am.empty_parse().channels(vec![attendance_channel]))
                            .flags(MessageFlags::EPHEMERAL)
                        })
                })
                .await;
            return Ok(());
        }

        let username = match &command.member {
            Some(member) => member.nick.as_deref().unwrap_or(&member.user.name),
            None => &command.user.name,
        };

        // Get the options from the command
        let wallet_address_option = command
            .data
            .options
            .get(0)
            .and_then(|o| o.value.as_ref())
            .and_then(|v| v.as_str());
        let number_of_tickets_option = command
            .data
            .options
            .get(1)
            .and_then(|o| o.value.as_ref())
            .and_then(|v| v.as_i64());

        // Check if the wallet address is valid and convert it to checksum format
        let _ = match wallet_address_option {
            // let wallet_address = match wallet_address_option {
            Some(addr) => match Address::from_str(addr) {
                Ok(address) => {
                    let checksummed = to_checksum(&address, None);
                    let content = format!(
                        "Hello {}!\nWe have already received your request of exchanging the Discord points into {} Tournament tickets from the wallet address {}.\nOnce your request is submitted, the points are subtracted immediately, and we will send you the Tournament ticket(s) on the coming Thursday!\nPlease check your Tournament page on Thursday.\nFor any inquiries, please contact the Discord Admin.",
                        username,
                        number_of_tickets_option.unwrap_or(0),
                        checksummed
                    );
                    let _ = command
                        .create_interaction_response(&ctx.http, |r| {
                            r.kind(InteractionResponseType::ChannelMessageWithSource)
                                .interaction_response_data(|m| {
                                    m.content(content).flags(MessageFlags::EPHEMERAL)
                                })
                        })
                        .await;

                    Some(checksummed)
                }
                Err(_) => {
                    let _ = command
                        .create_interaction_response(&ctx.http, |r| {
                            r.kind(InteractionResponseType::ChannelMessageWithSource)
                                .interaction_response_data(|m| {
                                    m.content("Invalid wallet address! Please try again.")
                                        .flags(MessageFlags::EPHEMERAL)
                                })
                        })
                        .await;
                    None
                }
            },
            None => {
                let _ = command
                    .create_interaction_response(&ctx.http, |r| {
                        r.kind(InteractionResponseType::ChannelMessageWithSource)
                            .interaction_response_data(|m| {
                                m.content("No wallet address provided! Please try again.")
                                    .flags(MessageFlags::EPHEMERAL)
                            })
                    })
                    .await;
                None
            }
        };

        // Check if the number of tickets is valid
        let _ = match number_of_tickets_option {
            // let number_of_tickets = match number_of_tickets_option {
            Some(num) => num as u8,
            None => {
                command
                    .create_interaction_response(&ctx.http, |r| {
                        r.kind(InteractionResponseType::ChannelMessageWithSource)
                            .interaction_response_data(|m| {
                                m.content("No number of tickets provided! Please try again.")
                            })
                    })
                    .await?;

                return Ok(());
            }
        };
        Ok(())
    }
}

pub async fn run_discord_bot(token: &str, db: Database) -> tokio::task::JoinHandle<()> {
    let intents = GatewayIntents::GUILD_MESSAGES
        | GatewayIntents::MESSAGE_CONTENT
        | GatewayIntents::GUILDS
        | GatewayIntents::GUILD_INTEGRATIONS;
    let mut client = Client::builder(&token, intents)
        .event_handler(Handler { db })
        .await
        .expect("Error creating Discord client");

    let handler = tokio::spawn(async move {
        client.start().await.expect("Error starting Discord client");
    });

    handler
}
