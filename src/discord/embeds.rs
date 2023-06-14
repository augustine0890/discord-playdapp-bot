use serenity::builder::CreateEmbed;
use serenity::utils::Color;
use serenity::{
    model::{prelude::ChannelId, user::User},
    prelude::Context,
};
use tracing::{error, info};

use crate::database::models::Exchange;

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

pub async fn send_check_points(ctx: &Context, channel_id: ChannelId, user: &User, points: i32) {
    let thumbnail = user.face();
    let footer_text = format!("Given to {}", user.tag());
    let footer_icon_url = thumbnail.clone();

    let mut embed = CreateEmbed::default();
    embed
        .title("The Cumulative Points")
        .color(Color::new(0x00AAFF))
        .thumbnail(thumbnail)
        .footer(|f| f.text(footer_text).icon_url(footer_icon_url))
        .timestamp(chrono::Utc::now().to_rfc3339());

    embed.field("Points", format!("{:?}", points), true);

    if let Err(why) = channel_id
        .send_message(&ctx.http, |m| m.set_embed(embed))
        .await
    {
        info!("Error sending message: {:?}", why);
    }
}

// Helper function to format and send a message to a Discord channel
pub async fn send_message(ctx: &Context, channel: ChannelId, content: String) {
    // Try to send the message
    if let Err(why) = channel.say(&ctx.http, &content).await {
        // If an error occurs, log it
        error!("Error sending the reaction message: {:?}", why);
    }
}
