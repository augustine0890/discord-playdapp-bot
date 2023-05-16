use serde::Deserialize;
use std::env;
use std::fs;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub development: EnvConfig,
    pub production: EnvConfig,
}

#[derive(Debug, Deserialize)]
pub struct EnvConfig {
    pub discord_token: String,
    pub mongo_uri: String,
    pub discord_guild: String,
    pub attendance_channel: String,
}

impl Config {
    pub fn new(file_path: &str) -> Result<EnvConfig, Box<dyn std::error::Error>> {
        let contents = fs::read_to_string(file_path)?;
        let config: Self = serde_yaml::from_str(&contents)?;

        let environment = env::var("APP_ENV").unwrap_or_else(|_| "production".to_string());
        let env_config = match environment.as_str() {
            "development" => config.development,
            _ => config.production,
        };

        env::set_var("DISCORD_TOKEN", &env_config.discord_token);
        env::set_var("MONGO_URI", &env_config.mongo_uri);
        env::set_var("DISCORD_GUILD", &env_config.discord_guild);
        env::set_var("ATTENDANCE_CHANNEL", &env_config.attendance_channel);

        Ok(env_config)
    }
}
