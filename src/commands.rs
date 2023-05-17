use serenity::builder;
use serenity::model::application::command::CommandOptionType;

pub fn exchange(
    command: &mut builder::CreateApplicationCommand,
) -> &mut builder::CreateApplicationCommand {
    command
        .name("exchange")
        .description("Exchange tickets")
        .create_option(|option| {
            option
                .name("wallet_address")
                .description("Your wallet address")
                .kind(CommandOptionType::String)
                .required(true)
        })
        .create_option(|option| {
            option
                .name("number_of_tickets")
                .description("Number of tickets to exchange")
                .kind(CommandOptionType::Integer)
                .required(true)
        })
}
