use config::Config;
use database::Database;

mod config;
mod database;

#[tokio::main]
async fn main() {
    let config = Config::new("config.yaml").expect("Failed to read configuration file");
    let _ = Database::new(&config.mongo_uri)
        .await
        .expect("Failed to connect to database");
    println!("Connected to database")
}
