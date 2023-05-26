use ethers::types::Address;
use ethers::utils::to_checksum;
use serenity::builder::CreateEmbed;
use serenity::model::user::User;
use serenity::utils::Color;
use serenity::{
    async_trait,
    model::application::command::Command,
    model::application::interaction::application_command::ApplicationCommandInteraction,
    model::application::interaction::{Interaction, InteractionResponseType},
    model::channel::Message as DiscordMessage,
    model::gateway::{GatewayIntents, Ready},
    model::id::ChannelId,
    model::prelude::interaction::MessageFlags,
    prelude::*,
};

use chrono::Utc;
use std::env;
use std::str::FromStr;
use std::sync::Arc;
use tracing::{error, info};

use crate::commands;
use crate::database::models::{Exchange, ExchangeStatus};
use crate::database::mongo::MongoDB;
use crate::scheduler::send_daily_report;
use crate::util::{self, filter_guilds};

pub struct Handler {
    pub db: MongoDB,
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
        // Filter out unwanted guilds, leaving those not in the allowed list
        filter_guilds(&ctx, ready).await;

        // Setup global commands, deleting the "exchange" command if it exists and recreating it
        setup_global_commands(&ctx).await;
    }

    async fn message(&self, ctx: Context, msg: DiscordMessage) {
        if let Err(why) = self.handle_records_command(&msg, &ctx).await {
            println!("Error handling records command: {:?}", why);
        }
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

        // Except Thursday for requesting the exchange
        if util::is_thu() {
            let _ = command
                .create_interaction_response(&ctx.http, |r| {
                    r.kind(InteractionResponseType::ChannelMessageWithSource)
                        .interaction_response_data(|m| {
                            m.content("Submission of request is only available on Mon-Wed, Fri-Sun.\nPlease submit again tomorrow.")
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
        let wallet_address = match wallet_address_option {
            Some(addr) => match Address::from_str(addr) {
                Ok(address) => {
                    let checksummed = to_checksum(&address, None);
                    Some(checksummed)
                }
                Err(_) => {
                    command
                        .create_interaction_response(&ctx.http, |r| {
                            r.kind(InteractionResponseType::ChannelMessageWithSource)
                                .interaction_response_data(|m| {
                                    m.content("Invalid wallet address! Please try again.")
                                        .flags(MessageFlags::EPHEMERAL)
                                })
                        })
                        .await?;

                    return Ok(());
                }
            },
            None => {
                command
                    .create_interaction_response(&ctx.http, |r| {
                        r.kind(InteractionResponseType::ChannelMessageWithSource)
                            .interaction_response_data(|m| {
                                m.content("No wallet address provided! Please try again.")
                                    .flags(MessageFlags::EPHEMERAL)
                            })
                    })
                    .await?;

                return Ok(());
            }
        };

        // Check if the number of tickets is valid
        let number_of_tickets = match number_of_tickets_option {
            Some(num) => num,
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

        // Check if the user has enough points
        let required_points = number_of_tickets as i32 * 1000;

        let user_points = self
            .db
            .get_user_points(&command.user.id.to_string())
            .await
            .unwrap_or_default();
        if user_points < required_points {
            command
                .create_interaction_response(&ctx.http, |r| {
                    r.kind(InteractionResponseType::ChannelMessageWithSource)
                        .interaction_response_data(|m| {
                            m.content("Sorry! You do not have enough points to exchange. Try to earn more points! 🏋️‍♂️💪🏋️‍♀️")
                                .flags(MessageFlags::EPHEMERAL)
                        })
                })
                .await?;

            return Ok(());
        }

        // Subtract the required points from the user's points
        self.db
            .subtract_user_points(&command.user.id.to_string(), required_points)
            .await?;

        const ITEM_TICKET: &str = "ticket";
        // Create an Exchange record
        let exchange = Exchange {
            id: None,
            dc_id: command.user.id.to_string(),
            dc_username: command.user.name.to_string(),
            wallet_address: wallet_address.clone(), // If it can be `None` and it's an error case
            item: ITEM_TICKET.to_string(),
            quantity: number_of_tickets,
            status: ExchangeStatus::Submitted,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        // Add the exchange record to the database
        if let Err(why) = self.db.add_exchange_record(exchange).await {
            error!("Error adding exchange record: {}", why);
        }

        // Send the hidden acknowledge message
        let content = format!(
            "Hello {}!👋🏻 \nWe have already received your request of exchanging the Discord points into **{} Tournament ticket(s)** from the wallet address **{}**.\nOnce your request is submitted, the points are subtracted immediately, and we will send you the Tournament ticket(s) on the coming **Thursday**!🤩 \nPlease check your Tournament page on Thursday.\nFor any inquiries, please contact the Discord Admin.🙌🏻",
            username,
            number_of_tickets,
            wallet_address.unwrap()
        );
        let _ = command
            .create_interaction_response(&ctx.http, |r| {
                r.kind(InteractionResponseType::ChannelMessageWithSource)
                    .interaction_response_data(|m| {
                        m.content(content).flags(MessageFlags::EPHEMERAL)
                    })
            })
            .await;

        // Send a public message to the channel
        if let Err(why) = command
            .channel_id
            .say(
                &ctx.http,
                format!(
                    "🥳 <@{}> just exchanged {} points to {} Tournament ticket(s)! 🎟️",
                    command.user.id, // Make sure to use the user's ID
                    number_of_tickets * 1000,
                    number_of_tickets
                ),
            )
            .await
        {
            error!("Error sending message: {}", why);
        }

        Ok(())
    }

    pub async fn handle_records_command(
        &self,
        msg: &DiscordMessage,
        ctx: &Context,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Set channel ID
        let attendance_channel: ChannelId = match env::var("ATTENDANCE_CHANNEL") {
            Ok(channel_str) => match channel_str.parse::<u64>() {
                Ok(channel_id) => ChannelId(channel_id),
                Err(_) => panic!("Failed to parse ATTENDANCE_CHANNEL as u64"),
            },
            Err(_) => panic!("ATTENDANCE_CHANNEL not found in environment"),
        };
        if msg.channel_id != attendance_channel {
            return Ok(());
        }

        if msg.content == "!cr" || msg.content == "!check-records" {
            let records = self.db.get_user_records(msg.author.id.to_string()).await?;
            if records.is_empty() {
                msg.reply(
                    &ctx.http,
                    format!(
                        "{} No Points Exchange Records found. 🔍\nPlease type “/exchange” to exchange your points to items. 🎁",
                        msg.author.mention()
                    ),
                )
                .await?;
                return Ok(());
            }

            let user: &User = &msg.author;
            let user_points = self
                .db
                .get_user_points(&msg.author.id.to_string())
                .await
                .unwrap_or_default();
            send_records_to_discord(&records, ctx, msg.channel_id, user, user_points).await;
        }
        Ok(())
    }
}

pub async fn run_discord_bot(token: &str, db: MongoDB) -> tokio::task::JoinHandle<()> {
    // Define the necessary gateway intents
    let intents = GatewayIntents::GUILD_MESSAGES
        | GatewayIntents::MESSAGE_CONTENT
        | GatewayIntents::GUILDS
        | GatewayIntents::GUILD_INTEGRATIONS;
    // Build the Discord client with the token, intents and event handler
    let client = Client::builder(&token, intents)
        .event_handler(Handler { db })
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
        send_daily_report(http, channel_id).await;

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

pub async fn send_records_to_discord(
    records: &[Exchange],
    ctx: &Context,
    channel_id: ChannelId,
    user: &User,
    points: i32,
) {
    let mut embed = CreateEmbed::default();
    for record in records {
        let title = format!("{}'s Exchange Records", record.dc_username.clone());
        let description = format!("Here is your Exchange Record of Discord Points.\nYour current remaining points is **{}**.", points);
        let items = format!("{} {}(s) 🎟️", record.quantity, record.item);

        embed
            .title(title)
            .description(description)
            .field("Item", items, true)
            .field("Status", format!("{:?}", record.status), true)
            .field(
                "Time (UTC)",
                record.updated_at.format("%Y-%m-%d %H:%M"),
                true,
            )
            .color(Color::new(0x00FA9A)) // note: Colour is the UK spelling of Color
            .thumbnail(user.face())
            .footer(|f| {
                f.text(format!("Given to {}", user.tag()))
                    .icon_url(user.face())
            })
            .timestamp(chrono::Utc::now().to_rfc3339());
    }

    if let Err(why) = channel_id
        .send_message(&ctx.http, |m| m.set_embed(embed))
        .await
    {
        println!("Error sending message: {:?}", why);
    }
}
