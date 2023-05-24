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
#### Build and Run as Native
1. Install Rust and necessary packages:
    - Download and install Rust: `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`
    - Add cargo's bin directory to the PATH environment variable: `source $HOME/.cargo/env`
    - Check Rust installation:
        - `rustc --version`
        - `cargo --version`
        - `rustup --version`
    - The build-essential package includes a number of tools necessary for compiling software fro- source on Ubuntu, including gcc (the GNU C compiler), make (a utility for directin- compilation), and other necessary utilities.
    - Install necessary C build tools:
        - `sudo apt update`
        - `sudo apt install build-essential`
    - The `pkg-config` and cryptography OpenSSL library: `sudo apt install pkg-config libssl-dev`
    - Install `sccache` tool that replaces compiler executable: `cargo install sccache`
2. Run the Application:
    - Execute permissions: `chmod +x start_app.sh`
    - Run the application as background: `./start_app.sh`
3. Check the Application:
    - Check the application is running: `ps -p $(cat discord-playdapp-bot.pid)`
    - View the application's output:
        - `cat discord-playdapp-bot.log`
        - Or for real-time output: `tail -n 20 -f discord-playdapp-bot.log`
    - To stop the application (use `kill` with PID): `kill $(cat discord-playdapp-bot.pid)`
    - (Optional) Rename the log files
    - `mv discord-playdapp-bot.log discord-playdapp-bot.log.datetime`
    - Get the start time of a process (application):
        - `ps -p $(cat discord-playdapp-bot.pid) -o lstart=`
    - Display live view of process's resource usage
        - `top -p $(cat discord-playdapp-bot.pid)`
        - Display the CPU and memory usage: 
            - `ps -p $(cat discord-playdapp-bot.pid) -o %cpu,%mem,cmd`
            - If the `%CPU` is over 100% that are running on multiple cores.

#### Build Docker
- Build the Docker image
    - `export DOCKER_BUILDKIT=1`
    - `docker build -t discord-playdapp-bot .`
- Run the Docker container
    - `docker run -d discord-playdapp-bot` --> run in background
    - `docker run --name discord-playdapp-bot -it --rm discord-playdapp-bot`
- Run the container in development enviroment
    - `docker run --env APP_ENV=development discord-playdapp-bot`
- Use sccache: sccache is a shared compilation cache that can help speed up Rust builds.
- Enable BuildKit by setting the environment variable:
    - `export DOCKER_BUILDKIT=1`
- Check the number of CPU cores on Ubuntu using: `lscpu | grep '^CPU(s):'`
- Build an image using the Dockerfile `named`:
    - `export DOCKER_BUILDKIT=1`
    - `docker build -t discord-playdapp-bot -f Dockerfile.optimized .`
    - `and run docker: docker run -d discord-playdapp-bot`
### Adding to your server
- Before you can use the bot, you must add it to your Discord server. Follow the official [Discord](https://discord.com/developers/docs/topics/oauth2#bots) guide to do this. You will need to know your bot's Client ID.

## Contributing
- Pull requests are welcome. For major changes, please open an issue first to discuss what you would like to change.

## License
- This project is licensed under the MIT License - see the [LICENSE](https://mit-license.org/) file for details.