use bson::Bson;
use chrono::{Duration, Utc};
use futures::stream::StreamExt;
use mongodb::bson::{doc, DateTime};
use mongodb::error::Error;
use mongodb::error::Result as MongoResult;
use mongodb::results::DeleteResult;
use mongodb::{
    options::{ClientOptions, FindOneOptions, FindOptions},
    Client, Database,
};
use tracing::error;

use crate::util::{generate_numbers, get_week_number};

use super::models::{Activity, ActivityType, Exchange, ExchangeStatus, LottoDraw};

#[derive(Clone)]
pub struct MongoDB {
    db: Database,
}

impl MongoDB {
    pub async fn new(uri: &str) -> Result<Self, Error> {
        let mut client_options = ClientOptions::parse(uri).await?;
        client_options.connect_timeout = Some(std::time::Duration::from_secs(10));

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

    pub async fn adjust_user_points(&self, user_id: &str, points: i32) -> MongoResult<()> {
        let user_collection = self.db.collection::<mongodb::bson::Document>("users");
        let filter = doc! {"_id": user_id};
        let update = doc! {"$inc": {"points": points }};
        if let Err(e) = user_collection.update_one(filter, update, None).await {
            error!("Error updating user points: {}", e);
        }
        Ok(())
    }

    pub async fn get_user_records(&self, dc_id: u64) -> Result<Vec<Exchange>, Error> {
        let exchange_collection = self.db.collection::<mongodb::bson::Document>("exchange");
        let filter = doc! {
            "dcId": dc_id as i64,
            // "status": { "$in": [Bson::String(ExchangeStatus::Submitted.to_string()), Bson::String(ExchangeStatus::Processing.to_string())] }
        };
        let options = FindOptions::builder()
            .projection(doc! {
                "_id": 0,
                "dcUsername": 1,
                "item": 1,
                "quantity": 1,
                "status": 1,
                "updatedAt": 1
            })
            .limit(8) // Limit the number of documents returned
            .sort(doc! {"updatedAt": -1}) // Order by 'updatedAt' in descending order
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

    pub async fn update_all_processing_to_completed(&self) -> Result<(), Error> {
        let exchange_collection = self.db.collection::<mongodb::bson::Document>("exchange");
        let filter = doc! { "status": Bson::String(ExchangeStatus::Processing.to_string()) };
        let update = doc! { "$set": { "status": Bson::String(ExchangeStatus::Completed.to_string())}, "$currentDate": { "updatedAt": true }};
        exchange_collection
            .update_many(filter, update, None)
            .await?;

        Ok(())
    }

    pub async fn clean_documents(&self) -> Result<DeleteResult, Error> {
        let activity_collection = self.db.collection::<mongodb::bson::Document>("activity");
        let about_five_weeks_ago = Utc::now() - Duration::weeks(5);
        let about_five_weeks_ago_bson = DateTime::from_chrono(about_five_weeks_ago);

        let delete_result = activity_collection
            .delete_many(
                doc! { "createdAt": { "$lt": about_five_weeks_ago_bson} },
                None,
            )
            .await?;

        Ok(delete_result)
    }

    pub async fn add_react_poll_activity(&self, new_activity: Activity) -> Result<bool, Error> {
        let activity_collection = self.db.collection::<mongodb::bson::Document>("activity");
        let today = Utc::now().date_naive().and_hms_opt(0, 0, 0).unwrap();
        let datetime_utc: chrono::DateTime<Utc> = chrono::DateTime::from_utc(today, Utc);

        // Filter to match activities by the same user, of the same type, on the same day
        let filter_today = doc! {
            "dcId": new_activity.dc_id as i64,
            "activity": Bson::String(ActivityType::Poll.to_string()),
            "createdAt": { "$gte": datetime_utc }
        };

        // Count the total number of activities today directly
        let total_count_today = activity_collection
            .count_documents(filter_today, None)
            .await?;

        // If the total count today is already 2, return false
        if total_count_today >= 2 {
            return Ok(false);
        }

        // Filter to match activities by the same user with the same message id, regardless of the day
        let filter_message_id = doc! {
            "dcId": new_activity.dc_id as i64,
            "activity": Bson::String(ActivityType::Poll.to_string()),
            "messageId": new_activity.message_id.unwrap()
        };

        // Count if there is already an activity with the same message id directly
        let has_same_message_id = activity_collection
            .count_documents(filter_message_id, None)
            .await?
            > 0;

        // If there is already an activity with the same message id, return false
        if has_same_message_id {
            return Ok(false);
        }

        // If conditions are met, add the new activity
        let new_activity_doc = bson::to_bson(&new_activity)?.as_document().unwrap().clone();
        activity_collection
            .insert_one(new_activity_doc, None)
            .await?;

        Ok(true)
    }

    pub async fn add_reaction_activity(&self, activity: Activity) -> Result<bool, Error> {
        let activity_collection = self.db.collection::<mongodb::bson::Document>("activity");
        let today = Utc::now().date_naive().and_hms_opt(0, 0, 0).unwrap();
        let datetime_utc: chrono::DateTime<Utc> = chrono::DateTime::from_utc(today, Utc);

        let reaction = &activity.activity.unwrap();
        let filter_today = doc! {
            "dcId": activity.dc_id as i64,
            "activity": reaction.to_string(),
            "createdAt": { "$gte": datetime_utc }
        };

        let record_count = activity_collection
            .count_documents(filter_today, None)
            .await?;

        let activity_doc = bson::to_bson(&activity)?.as_document().unwrap().clone();
        match *reaction {
            ActivityType::React if record_count > 4 => return Ok(false),
            ActivityType::Receive if record_count > 9 => return Ok(false),
            _ => {}
        }
        activity_collection.insert_one(activity_doc, None).await?;

        Ok(true)
    }

    pub async fn add_weekly_draw(&self) -> MongoResult<()> {
        let numbers = generate_numbers();
        let (year, week) = get_week_number();

        let lotto_draw_collection = self.db.collection::<mongodb::bson::Document>("lottodraw");
        let filter = doc! {
            "year": year,
            "weekNumber": week
        };

        // Check if a document for this week already exists
        match lotto_draw_collection.find_one(filter.clone(), None).await? {
            // If it does not exist, insert a new one
            None => {
                let draw = LottoDraw {
                    id: None,
                    year,
                    numbers,
                    week_number: week,
                    date: Utc::now(),
                    ..Default::default()
                };

                let draw_doc = bson::to_bson(&draw)
                    .expect("Failed to serialize")
                    .as_document()
                    .cloned()
                    .expect("Expected BSON Document");

                lotto_draw_collection.insert_one(draw_doc, None).await?;
            }
            // If it exists, do nothing
            Some(_) => {}
        }

        Ok(())
    }
}
