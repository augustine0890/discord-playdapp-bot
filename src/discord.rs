use ethers::types::Address;
use ethers::utils::to_checksum;
use serenity::builder::CreateEmbed;
use serenity::model::prelude::GuildId;
use serenity::model::user::User;
use serenity::utils::Color;
use serenity::{
    async_trait,
    model::application::command::Command,
    model::application::interaction::application_command::ApplicationCommandInteraction,
    model::application::interaction::{Interaction, InteractionResponseType},
    model::channel::Message as DiscordMessage,
    model::gateway::{GatewayIntents, Ready},
    model::id::{ChannelId, UserId},
    model::prelude::interaction::MessageFlags,
    model::prelude::Reaction,
    prelude::*,
};

use chrono::Utc;
use std::env;
use std::str::FromStr;
use std::sync::Arc;
use tracing::{error, info};

use crate::commands;
use crate::config::EnvConfig;
use crate::database::models::{Activity, ActivityType, Exchange, ExchangeStatus};
use crate::database::mongo::MongoDB;
use crate::scheduler::send_daily_report;
use crate::util::{self, filter_guilds};

pub struct Handler {
    pub db: MongoDB,
    pub config: EnvConfig,
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
            error!("Error handling records command: {:?}", why);
        }
    }

    // When the reaction is added in Discord
    async fn reaction_add(&self, ctx: Context, add_reaction: Reaction) {
        // add activity points based on the reaction poll.
        if let Err(why) = self.poll_reaction(&ctx, &add_reaction).await {
            error!("Error adding reaction poll: {:?}", why);
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
            .adjust_user_points(&command.user.id.to_string(), -required_points)
            .await?;

        const ITEM_TICKET: &str = "ticket";
        // Create an Exchange record
        let exchange = Exchange {
            id: None,
            dc_id: command.user.id.into(),
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
            let records = self.db.get_user_records(msg.author.id.into()).await?;
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

    async fn poll_reaction(
        &self,
        ctx: &Context,
        add_reaction: &Reaction,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        const EASY_POLL: UserId = UserId(437618149505105920);
        const REWARD_POINTS: i32 = 15;

        // Specify the ID of the channel you want to send to.
        let attendance_channel: ChannelId = match self.config.attendance_channel.parse::<u64>() {
            Ok(channel_id) => ChannelId(channel_id),
            Err(_) => panic!("Failed to parse ATTENDANCE_CHANNEL as u64"),
        };

        let guild: GuildId = match self.config.discord_guild.parse::<u64>() {
            Ok(guild_id) => GuildId(guild_id),
            Err(_) => panic!("Failed to parse DISCORD_GUILD as u64"),
        };

        // Get the ID of the user who added the reaction.
        let user_id = match add_reaction.user_id {
            Some(user_id) => user_id,
            None => {
                return Err(Box::new(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "No user ID found for reaction",
                )));
            }
        };

        // Get the user who added the reaction.
        let user = user_id.to_user(&ctx).await?;
        let user_name = user.name;

        // Get the message id.
        let message_id = i64::from(add_reaction.message_id);

        // let emoji_name = add_reaction.emoji.name().unwrap_or_default().to_string();

        // Fetch the message that was reacted to.
        let message = add_reaction.message(&ctx).await?;
        // Get the guild id
        let guild_id = message.guild_id.unwrap_or_default();
        let message_channel_id = message.channel_id;

        // Get the author of the message.
        let author_id = message.author.id;

        // If the content is not created by EASY_POLL or if the user trying to access it is EASY_POLL, we terminate the function early.
        // This ensures only content from EASY_POLL is processed and that EASY_POLL cannot modify/access its own content.
        if author_id != EASY_POLL || user_id == EASY_POLL || user.bot || guild_id == guild {
            return Ok(());
        }

        // Create a new activity.
        let activity = Activity {
            id: None,
            dc_id: user_id.into(),
            dc_username: Some(user_name),
            activity: Some(ActivityType::Poll),
            reward: REWARD_POINTS,
            message_id: Some(message_id),
            created_at: Utc::now(),
            ..Default::default()
        };

        // Add the activity document to the database.
        if let Ok(true) = self.db.add_react_poll_activity(activity).await {
            // Convert the UserId to a string.
            let user_id_str = user_id.to_string();

            // Give points to the user.
            self.db
                .adjust_user_points(&user_id_str, REWARD_POINTS)
                .await?;

            // Format the message to send
            let content = format!(
                "<@{}> got 15 points from participating in the [Quiz & Poll] (https://discord.com/channels/{}/{}/{}) in <#{}> channel 👏🏻",
                user_id, guild_id, message_channel_id, message_id, message_channel_id
            );
            // Send the message
            if let Err(why) = attendance_channel.say(&ctx.http, &content).await {
                error!("Error sending the reaction poll message: {:?}", why);
            }
        }

        Ok(())
    }
}

pub async fn run_discord_bot(
    token: &str,
    db: MongoDB,
    config: EnvConfig,
) -> tokio::task::JoinHandle<()> {
    // Define the necessary gateway intents
    let intents = GatewayIntents::GUILD_MESSAGES
        | GatewayIntents::MESSAGE_CONTENT
        | GatewayIntents::GUILDS
        | GatewayIntents::GUILD_INTEGRATIONS
        | GatewayIntents::GUILD_MESSAGE_REACTIONS
        | GatewayIntents::DIRECT_MESSAGE_REACTIONS;
    // Build the Discord client with the token, intents and event handler
    let client = Client::builder(&token, intents)
        .event_handler(Handler { db, config })
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
    let title = format!("{}'s Exchange Records", user.name);
    let description = format!(
        "Here is your Exchange Record of Discord Points.\nYour current remaining points is **{}**.",
        points
    );
    let thumbnail = user.face();
    let footer_text = format!("Given to {}", user.tag());
    let footer_icon_url = thumbnail.clone();

    let mut embed = CreateEmbed::default();
    embed
        .title(title)
        .description(description)
        .color(Color::new(0x00FA9A))
        .thumbnail(thumbnail)
        .footer(|f| f.text(footer_text).icon_url(footer_icon_url))
        .timestamp(chrono::Utc::now().to_rfc3339());

    for record in records {
        let items = format!("{} {}(s) 🎟️", record.quantity, record.item);
        embed
            .field("Item", items, true)
            .field("Status", format!("{:?}", record.status), true)
            .field(
                "Time (UTC)",
                record.updated_at.format("%Y-%m-%d %H:%M"),
                true,
            );
    }

    if let Err(why) = channel_id
        .send_message(&ctx.http, |m| m.set_embed(embed))
        .await
    {
        info!("Error sending message: {:?}", why);
    }
}
