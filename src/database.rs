use bson::Bson;
use chrono::Utc;
use futures::stream::StreamExt;
use mongodb::bson::{doc, oid::ObjectId};
use mongodb::error::Error;
use mongodb::error::Result as MongoResult;
use mongodb::{
    options::{ClientOptions, FindOneOptions, FindOptions},
    Client, Database,
};
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default)]
pub enum ExchangeStatus {
    #[default]
    Submitted,
    Processing,
    Completed,
}

impl fmt::Display for ExchangeStatus {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            ExchangeStatus::Submitted => write!(f, "Submitted"),
            ExchangeStatus::Processing => write!(f, "Processing"),
            ExchangeStatus::Completed => write!(f, "Completed"),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct Exchange {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,
    #[serde(skip_deserializing)]
    pub dc_id: String,
    pub dc_username: String,
    pub wallet_address: Option<String>,
    pub item: String,
    pub quantity: i64,
    pub status: ExchangeStatus,
    #[serde(rename = "createdAt", skip_deserializing)]
    #[serde(with = "bson::serde_helpers::chrono_datetime_as_bson_datetime")]
    pub created_at: chrono::DateTime<Utc>,
    #[serde(rename = "updatedAt")]
    #[serde(with = "bson::serde_helpers::chrono_datetime_as_bson_datetime")]
    pub updated_at: chrono::DateTime<Utc>,
}
#[derive(Clone)]
pub struct MongoDB {
    db: Database,
}

impl MongoDB {
    pub async fn new(uri: &str) -> Result<Self, Error> {
        let client_options = ClientOptions::parse(uri).await?;
        let client =
            Client::with_options(client_options).expect("Failed to connect to MongoDB client");

        Ok(MongoDB {
            db: client.database("discord-bot"),
        })
    }

    pub async fn add_exchange_record(&self, exchange: Exchange) -> MongoResult<()> {
        let exchange_collection = self.db.collection::<mongodb::bson::Document>("exchange");
        let exchange_doc = bson::to_bson(&exchange)
            .unwrap()
            .as_document()
            .unwrap()
            .clone();
        exchange_collection
            .insert_one(exchange_doc, None)
            .await
            .map(|_| ())
    }

    pub async fn get_user_points(&self, user_id: &str) -> MongoResult<i32> {
        let user_collection = self.db.collection::<mongodb::bson::Document>("users");
        let filter = doc! {"_id": user_id };
        let options = FindOneOptions::builder()
            .projection(doc! {"points": 1})
            .build();
        let result = user_collection.find_one(filter, options).await?;

        match result {
            Some(document) => {
                let points = document.get_i32("points").unwrap_or_default();
                Ok(points)
            }
            None => Ok(0), // Return 0 if the user is not found
        }
    }

    pub async fn subtract_user_points(&self, user_id: &str, points: i32) -> MongoResult<()> {
        let user_collection = self.db.collection::<mongodb::bson::Document>("users");
        let filter = doc! {"_id": user_id};
        let update = doc! {"$inc": {"points": -points }};
        user_collection
            .update_one(filter, update, None)
            .await
            .map(|_| ())
    }

    pub async fn get_user_records(&self, dc_id: String) -> Result<Vec<Exchange>, Error> {
        let exchange_collection = self.db.collection::<mongodb::bson::Document>("exchange");
        let filter = doc! {
            "dc_id": dc_id,
            "status": { "$in": [Bson::String(ExchangeStatus::Submitted.to_string()), Bson::String(ExchangeStatus::Processing.to_string())] }
        };
        let options = FindOptions::builder()
            .projection(doc! {
                "_id": 0,
                "dc_username": 1,
                "item": 1,
                "quantity": 1,
                "status": 1,
                "updatedAt": 1
            })
            .build();
        let mut cursor = exchange_collection.find(filter, options).await?;
        let mut results = Vec::new();
        while let Some(result) = cursor.next().await {
            match result {
                Ok(doc) => {
                    let exchange: Exchange = bson::from_bson(Bson::Document(doc))?;
                    results.push(exchange);
                }
                Err(e) => return Err(e),
            }
        }
        Ok(results)
    }

    pub async fn update_all_submitted_to_processing(&self) -> Result<(), Error> {
        let exchange_collection = self.db.collection::<mongodb::bson::Document>("exchange");
        let filter = doc! { "status": Bson::String(ExchangeStatus::Submitted.to_string()) };
        let update = doc! { "$set": { "status": Bson::String(ExchangeStatus::Processing.to_string())}, "$currentDate": { "updatedAt": true }};
        exchange_collection
            .update_many(filter, update, None)
            .await?;
        Ok(())
    }
}
