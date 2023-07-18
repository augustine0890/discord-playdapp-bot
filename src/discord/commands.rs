use super::embeds::{send_check_points, send_records_to_discord};
use crate::database::models::{Activity, ActivityType, Exchange, ExchangeStatus, LottoGuess};
use crate::discord::embeds::send_message;
use crate::util::{self, calculate_lotto_points, get_week_number, BAD_EMOJI};
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

impl Handler {
    pub async fn handle_exchange(
        &self,
        ctx: Context,
        command: ApplicationCommandInteraction,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Check for the correct channel
        let attendance_channel_id = self.config.attendance_channel;
        let attendance_channel = ChannelId(attendance_channel_id);

        if command.channel_id != attendance_channel {
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

    pub async fn handle_lotto(
        &self,
        ctx: Context,
        command: ApplicationCommandInteraction,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let lotto_channel_id = self.config.lotto_channel;
        let lotto_channel = ChannelId(lotto_channel_id);

        if command.channel_id != lotto_channel {
            let _ = command
                .create_interaction_response(&ctx.http, |r| {
                    r.kind(InteractionResponseType::ChannelMessageWithSource)
                        .interaction_response_data(|m| {
                            m.content(format!(
                                "Please go to the <#{}> channel to participate in the LOTTO game 🎰",
                                lotto_channel
                            ))
                            .flags(MessageFlags::EPHEMERAL)
                        })
                })
                .await;
            return Ok(());
        }

        let user_name = match &command.member {
            Some(member) => member.nick.as_deref().unwrap_or(&member.user.name),
            None => &command.user.name,
        };

        let first_number = command
            .data
            .options
            .get(0)
            .and_then(|o| o.value.as_ref())
            .and_then(|v| v.as_i64())
            .unwrap_or(0) as i32;
        let second_number = command
            .data
            .options
            .get(1)
            .and_then(|o| o.value.as_ref())
            .and_then(|v| v.as_i64())
            .unwrap_or(0) as i32;
        let third_number = command
            .data
            .options
            .get(2)
            .and_then(|o| o.value.as_ref())
            .and_then(|v| v.as_i64())
            .unwrap_or(0) as i32;
        let fourth_number = command
            .data
            .options
            .get(3)
            .and_then(|o| o.value.as_ref())
            .and_then(|v| v.as_i64())
            .unwrap_or(0) as i32;

        let user_numbers = vec![first_number, second_number, third_number, fourth_number];

        // Fetch current week number
        let (year, current_week) = get_week_number();

        // Attempt to retrieve the draw numbers from the database
        let draw_numbers_result = self.db.get_lotto_draw(year, current_week).await;

        // Unwrap draw numbers or log error and return
        let draw_numbers = match draw_numbers_result {
            Ok(numbers) => numbers,
            Err(e) => {
                error!("Error fetching lotto draw numbers: {}", e);
                return Err(Box::new(e) as Box<dyn std::error::Error + Send + Sync>);
                // Return the error, stopping the function execution
            }
        };

        // Calculate matching numbers and corresponding reward points
        let (matches, reward_points) = calculate_lotto_points(&user_numbers, &draw_numbers);

        // Build a LottoGuess object with calculated data
        let guess = LottoGuess {
            id: None,
            dc_id: command.user.id.into(),
            dc_username: Some(user_name.to_string()),
            numbers: user_numbers,
            year,
            week_number: current_week,
            matched_count: Some(matches.try_into().unwrap()), // Convert matches to i32
            is_any_matched: Some(matches > 0), // Boolean flag indicating any match found
            points: Some(reward_points),       // Reward points
            dm_sent: Some(false),              // Flag indicating if a direct message was sent
            created_at: Utc::now(),            // Current timestamp
            updated_at: Utc::now(),
        };

        // Try to add the lotto guess to the database
        match self.db.add_lotto_guess(guess).await {
            Ok(true) => {
                // If we reach here, it means the lotto guess was successfully added to the database.
                let content = format!(
            "You have chosen {}, {}, {}, {} for the lotto 🎰\nThe results will be revealed on the upcoming Monday at 03:00 (UTC+0) 😎\nGood luck! 🍀",
            first_number, second_number, third_number, fourth_number
        );

                let _ = command
                    .create_interaction_response(&ctx.http, |r| {
                        r.kind(InteractionResponseType::ChannelMessageWithSource)
                            .interaction_response_data(|m| {
                                m.content(content).flags(MessageFlags::EPHEMERAL)
                            })
                    })
                    .await;
            }
            Ok(false) => {
                // User has already made 3 guesses this week.
                let content = "You have already made 3 guesses this week 😩 Please wait until next week to play again 💪🏻";

                let _ = command
                    .create_interaction_response(&ctx.http, |r| {
                        r.kind(InteractionResponseType::ChannelMessageWithSource)
                            .interaction_response_data(|m| {
                                m.content(content).flags(MessageFlags::EPHEMERAL)
                            })
                    })
                    .await;
            }
            Err(e) => {
                // An error occurred while adding the lotto guess to the database.
                error!("Error adding lotto guess to the database: {}", e);
            }
        }

        Ok(()) // Continue the function despite the outcome
    }

    // This function is responsible for handling record check commands.
    pub async fn handle_records_command(
        &self,
        msg: &DiscordMessage,
        ctx: &Context,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // If the message content is not a record check command, we ignore it and return early.
        if msg.content != "!cr" && msg.content != "!check-records" {
            return Ok(());
        }

        // Extract the guild ID from the configuration
        let guild: u64 = self.config.discord_guild;

        // Extract the guild ID from the message
        let guild_id = msg.guild_id.unwrap_or_default().0;

        // If the guild ID from the message does not match the guild ID from the configuration,
        // we ignore the message and return early.
        if guild_id != guild {
            return Ok(());
        }

        // Extract the attendance channel ID from the configuration
        let attendance_channel_id = self.config.attendance_channel;

        // Create a ChannelId instance from the attendance channel ID
        let attendance_channel = ChannelId(attendance_channel_id);

        // If the channel where the message was sent is not the attendance channel,
        // we ignore the message and return early.
        if msg.channel_id != attendance_channel {
            return Ok(());
        }

        // Extract the user who sent the message
        let user: &User = &msg.author;

        // Get the points of the user from the database
        let user_points = self
            .db
            .get_user_points(&msg.author.id.to_string())
            .await
            .unwrap_or_default();

        // Get the user's record from the database
        let records = self.db.get_user_records(msg.author.id.into()).await?;

        // If the user has no records,
        // reply with a message that no record was found and return early.
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

        // If the user has records, send these records to the attendance channel on Discord.
        send_records_to_discord(&records, ctx, msg.channel_id, user, user_points).await;

        Ok(())
    }

    pub async fn handle_points_command(
        &self,
        msg: &DiscordMessage,
        ctx: &Context,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Check if the command is meant for checking points.
        // If not, we simply return early without any operation.
        if msg.content != "!cp" && msg.content != "!check-points" {
            return Ok(());
        }

        // Extract the configured guild ID for the bot.
        let guild: u64 = self.config.discord_guild;

        // Extract the guild ID from the message.
        let guild_id = msg.guild_id.unwrap_or_default().0;

        // If the guild ID from the message doesn't match the configured guild,
        // we don't process the command and return early.
        if guild_id != guild {
            return Ok(());
        }

        // Extract the attendance channel ID from the configuration.
        let attendance_channel_id = self.config.attendance_channel;
        let attendance_channel = ChannelId(attendance_channel_id);

        // Extract the user who sent the message.
        let user: &User = &msg.author;

        // Retrieve the user's points from the database.
        let user_points = self
            .db
            .get_user_points(&msg.author.id.to_string())
            .await
            .unwrap_or_default();

        // Check if the message was sent in the attendance channel.
        // If not, we reply with a message directing the user to the attendance channel.
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

        // If the message was in the attendance channel,
        // we send the user's points information to the channel.
        send_check_points(ctx, msg.channel_id, user, user_points).await;

        Ok(())
    }

    pub async fn poll_reaction(
        &self,
        ctx: &Context,
        add_reaction: &Reaction,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Constants for the poll bot and the reward points.
        const EASY_POLL: UserId = UserId(437618149505105920);
        const REWARD_POINTS: i32 = 15;

        // Extract the attendance channel ID from the configuration.
        let attendance_channel_id = self.config.attendance_channel;
        let attendance_channel = ChannelId(attendance_channel_id);

        // Extract the configured guild ID.
        let guild: u64 = self.config.discord_guild;

        // Try to get the user who added the reaction.
        // If the user cannot be found, return an error.
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

        // Extract the message ID from the reaction.
        let message_id = i64::from(add_reaction.message_id);

        // Try to get the guild ID from the reaction.
        // If the guild ID cannot be found, default to 0.
        let guild_id = match add_reaction.guild_id {
            Some(id) => id.0,
            None => 0,
        };

        // Fetch the message that was reacted to.
        let message = add_reaction.message(&ctx).await?;
        let message_channel_id = message.channel_id;

        // Get the author of the message.
        let author_id = message.author.id;

        // Ignore the reaction if it's from a bot, not from the guild or not from EASY_POLL.
        if user.bot || guild_id != guild || author_id != EASY_POLL || user_id == EASY_POLL {
            return Ok(());
        }

        // Construct a new activity to record in the database.
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
        // If the document was successfully added, award points to the user and send a confirmation message.
        if let Ok(true) = self.db.add_react_poll_activity(activity).await {
            // Adjust the user's points in the database.
            self.db
                .adjust_user_points(&user_id.to_string(), REWARD_POINTS)
                .await?;

            // Prepare the content for the confirmation message.
            let content = format!(
                "<@{}> got 15 points from participating in the [Quiz & Poll] (https://discord.com/channels/{}/{}/{}) in <#{}> channel 👏🏻",
                user_id, guild_id, message_channel_id, message_id, message_channel_id
            );

            // Send the message.
            // Log an error if the message couldn't be sent.
            if let Err(why) = attendance_channel.say(&ctx.http, &content).await {
                error!("Error sending the reaction poll message: {:?}", why);
            }
        }

        Ok(())
    }

    /// This function handles reaction activities in the Discord server.
    /// It grants or deducts points based on the type of the emoji in the reaction.
    pub async fn reaction_activity(
        &self,
        ctx: &Context,
        reaction: &Reaction,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Extract the configured guild ID.
        let guild: u64 = self.config.discord_guild;

        // Extract the guild ID from the reaction. If it's not the same as the configured guild, return early.
        let guild_id = reaction.guild_id.unwrap_or_default().0;
        if guild_id != guild {
            return Ok(());
        }

        // Constants for points and channel IDs.
        const DEDUCT_POINTS: i32 = -10;
        const REACT_POINTS: i32 = 3;
        const RECEIVE_POINTS: i32 = 10;
        const ANNOUNCEMENT_CHANNEL: ChannelId = ChannelId(537522976963166218);

        // Extract the attendance channel ID from the configuration.
        let attendance_channel_id = self.config.attendance_channel;
        let attendance_channel = ChannelId(attendance_channel_id);

        // Try to extract the user ID from the reaction. If it cannot be found, return an error.
        let user_id = reaction.user_id.ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::Other, "No user ID found for reaction")
        })?;

        // Fetch the user who reacted and the message that was reacted to.
        let user = user_id.to_user(&ctx).await?;
        let user_name = &user.name;
        let message_id = i64::from(reaction.message_id);
        let message = reaction.message(&ctx).await?;
        let message_channel_id = message.channel_id;

        // Try to extract the name of the emoji used in the reaction. If it cannot be found, return an error.
        let emoji_name = match &reaction.emoji {
            ReactionType::Custom { name, .. } => name.as_ref().map(|s| s.as_str()),
            ReactionType::Unicode(s) => Some(s.as_str()),
            _ => None,
        };
        let emoji_name = emoji_name.ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::Other, "Emoji name not found")
        })?;

        // If a bad emoji was used, deduct points from the user and notify them.
        if BAD_EMOJI.contains(emoji_name) {
            self.db
                .adjust_user_points(&user_id.to_string(), DEDUCT_POINTS)
                .await?;

            let content = format!(
                "<@{}> got 10 points deducted for reacting {} in the <#{}> channel.",
                user_id, emoji_name, attendance_channel.0
            );

            send_message(ctx, attendance_channel, content).await;

            return Ok(());
        }

        // Fetch the author of the message.
        let author = message.author;

        // If the reaction is from a bot, to a bot's message, from the author themselves, or in the attendance channel, ignore it.
        if message_channel_id == attendance_channel || author.bot || user.bot || (author == user) {
            return Ok(());
        }

        // Construct a new "React" activity.
        let activity = Activity {
            id: None,
            dc_id: user_id.into(),
            dc_username: Some(user_name.to_string()),
            channel_id: Some(message_channel_id.into()),
            activity: Some(ActivityType::React),
            reward: REACT_POINTS,
            message_id: Some(message_id),
            emoji: Some(emoji_name.to_string()),
            created_at: Utc::now(),
            ..Default::default()
        };

        // Add the activity to the database and grant points to the user.
        if let Ok(true) = self.db.add_reaction_activity(activity).await {
            let user_id_str = user_id.to_string();

            self.db
                .adjust_user_points(&user_id_str, REACT_POINTS)
                .await?;

            let content = format!(
            "<@{}> got 3 points from reacting {} on (https://discord.com/channels/{}/{}/{}) in the <#{}> channel.",
            user_id, emoji_name, guild_id, message_channel_id, message_id, message_channel_id
        );

            send_message(ctx, attendance_channel, content).await;
        }

        // If the message is from the announcement channel, return early (no points are granted for reactions in this channel).
        if message_channel_id == ANNOUNCEMENT_CHANNEL {
            return Ok(());
        }

        // Construct a new "Receive" activity.
        let activity = Activity {
            id: None,
            dc_id: author.id.into(),
            dc_username: Some(author.name.to_string()),
            channel_id: Some(message_channel_id.into()),
            activity: Some(ActivityType::Receive),
            reward: RECEIVE_POINTS,
            message_id: Some(message_id),
            emoji: Some(emoji_name.to_string()),
            created_at: Utc::now(),
            ..Default::default()
        };

        // Add the activity to the database and grant points to the author of the message.
        if let Ok(true) = self.db.add_reaction_activity(activity).await {
            let author_id_str = author.id.to_string();

            self.db
                .adjust_user_points(&author_id_str, RECEIVE_POINTS)
                .await?;

            let content = format!(
            "<@{}> got 10 points from <@{}>'s reaction {} on (https://discord.com/channels/{}/{}/{}) in the <#{}> channel.",
            author.id, user_id, emoji_name, guild_id, message_channel_id, message_id, message_channel_id
        );

            send_message(ctx, attendance_channel, content).await;
        }

        Ok(())
    }
}
