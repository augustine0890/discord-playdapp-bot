# Discord PlayDapp Bot
- This project is a Discord bot built with Rust that allows users to interact with a MongoDB database. The bot responds to several slash commands, including `/exchange`, `/attendance`, `/points`, and `/ranking`.

## Features
- Exchange tickets: Users can use the `/exchange` command followed by their wallet address and the number of tickets they want to exchange. The bot will store the exchange request in a MongoDB database.
- Attendance: Coming soon.
- Points: Coming soon.
- Ranking: Coming soon.
## Setup
### Requirements
- Rust
- MongoDB
### Configuration
- The bot requires a YAML configuration file named `config.yaml` in the project root directory. This file should contain the following:
### Building and Running
### Adding to your server
- Before you can use the bot, you must add it to your Discord server. Follow the official [Discord](https://discord.com/developers/docs/topics/oauth2#bots) guide to do this. You will need to know your bot's Client ID.

## Contributing
- Pull requests are welcome. For major changes, please open an issue first to discuss what you would like to change.

## License
- This project is licensed under the MIT License - see the [LICENSE](https://mit-license.org/) file for details.