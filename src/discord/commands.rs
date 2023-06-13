use super::embeds::{send_check_points, send_records_to_discord};
use crate::database::models::{Activity, ActivityType, Exchange, ExchangeStatus};
use crate::util::{self, BAD_EMOJI};
use chrono::Utc;
use ethers::types::Address;
use ethers::utils::to_checksum;
use std::str::FromStr;
use tracing::error;

use serenity::{
    model::channel::Message as DiscordMessage,
    model::prelude::interaction::{
        application_command::ApplicationCommandInteraction, InteractionResponseType, MessageFlags,
    },
    model::prelude::{ChannelId, Reaction, ReactionType, UserId},
    model::user::User,
    prelude::{Context, Mentionable},
};

use super::handler::Handler;
use std::env;

impl Handler {
    pub async fn handle_exchange(
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

    pub async fn handle_points_command(
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

        let user: &User = &msg.author;
        let user_points = self
            .db
            .get_user_points(&msg.author.id.to_string())
            .await
            .unwrap_or_default();

        if msg.content != "!cp" && msg.content != "!check-point" {
            return Ok(());
        }

        if msg.channel_id != attendance_channel {
            msg.reply(
                &ctx.http,
                format!(
                    "{} Please go to the <#{}> channel for Daily Attendance and Points Checking.",
                    msg.author.mention(),
                    attendance_channel
                ),
            )
            .await?;
            return Ok(());
        }

        send_check_points(ctx, msg.channel_id, user, user_points).await;

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

        let user: &User = &msg.author;
        let user_points = self
            .db
            .get_user_points(&msg.author.id.to_string())
            .await
            .unwrap_or_default();

        if msg.content != "!cr" && msg.content != "!check-record" {
            return Ok(());
        }

        if msg.channel_id != attendance_channel {
            return Ok(());
        }

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

        send_records_to_discord(&records, ctx, msg.channel_id, user, user_points).await;

        Ok(())
    }

    pub async fn poll_reaction(
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

        let guild: u64 = match self.config.discord_guild.parse::<u64>() {
            Ok(guild_id) => guild_id,
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

        // Get the guild id
        let guild_id = match add_reaction.guild_id {
            Some(id) => id.0, // Get the inner u64 value
            None => {
                // Handle the case where there's no guild_id. This could involve returning an error or a dummy u64.
                // We'll return a dummy value (0) for this example.
                0
            }
        };

        // let emoji_name = add_reaction.emoji.name().unwrap_or_default().to_string();

        // Fetch the message that was reacted to.
        let message = add_reaction.message(&ctx).await?;
        let message_channel_id = message.channel_id;

        // Get the author of the message.
        let author_id = message.author.id;

        // Ignore the reaction if: it's not to a message from EASY_POLL, it's from EASY_POLL or a bot, or it's not from the specified guild.
        if author_id != EASY_POLL || user_id == EASY_POLL || user.bot || guild_id != guild {
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

    pub async fn reaction_activity(
        &self,
        ctx: &Context,
        reaction: &Reaction,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let attendance_channel: ChannelId = match self.config.attendance_channel.parse::<u64>() {
            Ok(channel_id) => ChannelId(channel_id),
            Err(_) => panic!("Failed to parse ATTENDANCE_CHANNEL as u64"),
        };

        let user_id = match reaction.user_id {
            Some(user_id) => user_id,
            None => {
                return Err(Box::new(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "No user ID found for reaction",
                )));
            }
        };

        let user_id_str = user_id.to_string();

        let emoji_name = match &reaction.emoji {
            ReactionType::Custom { name, .. } => name.as_ref().map(|s| s.as_str()),
            ReactionType::Unicode(s) => Some(s.as_str()),
            _ => None,
        };

        let emoji_name = match emoji_name {
            Some(emoji_name) => emoji_name,
            None => {
                return Err(Box::new(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "Emoji name not found",
                )));
            }
        };

        if BAD_EMOJI.contains(emoji_name) {
            const DEDUCT_POINTS: i32 = -10;
            self.db
                .adjust_user_points(&user_id_str, DEDUCT_POINTS)
                .await?;

            let content = format!(
                "<@{}> got 10 points deducted for reacting {} in the <#{}> channel.",
                user_id, emoji_name, attendance_channel.0
            );

            if let Err(why) = attendance_channel.say(&ctx.http, &content).await {
                error!("Error sending the reaction poll message: {:?}", why);
            }
        }

        Ok(())
    }
}
