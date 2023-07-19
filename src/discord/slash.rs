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
                .min_int_value(1)
                .max_int_value(256)
                .required(true)
        })
}

pub fn lotto(
    command: &mut builder::CreateApplicationCommand,
) -> &mut builder::CreateApplicationCommand {
    command
        .name("lotto")
        .description("Weekly Lottery")
        .create_option(|option| {
            option
                .name("1st_number")
                .description("The first number")
                .kind(CommandOptionType::Integer)
                .min_int_value(0)
                .max_int_value(9)
                .required(true)
        })
        .create_option(|option| {
            option
                .name("2nd_number")
                .description("The second number")
                .kind(CommandOptionType::Integer)
                .min_int_value(0)
                .max_int_value(9)
                .required(true)
        })
        .create_option(|option| {
            option
                .name("3rd_number")
                .description("The third number")
                .kind(CommandOptionType::Integer)
                .min_int_value(0)
                .max_int_value(9)
                .required(true)
        })
        .create_option(|option| {
            option
                .name("4th_number")
                .description("The fourth number")
                .kind(CommandOptionType::Integer)
                .min_int_value(0)
                .max_int_value(9)
                .required(true)
        })
}

pub fn lotto_guideine(
    command: &mut builder::CreateApplicationCommand,
) -> &mut builder::CreateApplicationCommand {
    command
        .name("lotto-guideline")
        .description("The official guideline for Weekly Lotto")
}
