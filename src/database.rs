#![allow(dead_code)]

use mongodb::error::Error;
use mongodb::{options::ClientOptions, Client};

#[derive(Clone)]
pub struct Database {
    client: Client,
}

impl Database {
    pub async fn new(uri: &str) -> Result<Self, Error> {
        let client_options = ClientOptions::parse(uri).await?;
        let client = Client::with_options(client_options)?;
        Ok(Database { client })
    }
}
