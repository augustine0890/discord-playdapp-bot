use chrono::Utc;
use mongodb::bson::{doc, oid::ObjectId};
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
    #[serde(rename = "dcId", skip_deserializing)]
    pub dc_id: u64,
    #[serde(rename = "dcUsername")]
    pub dc_username: String,
    #[serde(rename = "walletAddress")]
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

#[derive(Serialize, PartialEq, Deserialize, Clone, Copy, Debug)]
#[serde(rename_all = "lowercase")]
pub enum ActivityType {
    Attend,
    React,
    Receive,
    Awaken,
    Poll,
    Lotto,
}

impl fmt::Display for ActivityType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            ActivityType::Attend => write!(f, "attend"),
            ActivityType::React => write!(f, "react"),
            ActivityType::Receive => write!(f, "receive"),
            ActivityType::Awaken => write!(f, "awaken"),
            ActivityType::Poll => write!(f, "poll"),
            ActivityType::Lotto => write!(f, "lotto"),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct Activity {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,
    #[serde(rename = "dcId", skip_deserializing)]
    pub dc_id: u64,
    #[serde(rename = "dcUsername", skip_serializing_if = "Option::is_none")]
    pub dc_username: Option<String>,
    #[serde(rename = "channelId", skip_serializing_if = "Option::is_none")]
    pub channel_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub activity: Option<ActivityType>,
    pub reward: i32,
    #[serde(rename = "messageId", skip_serializing_if = "Option::is_none")]
    pub message_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub emoji: Option<String>,
    #[serde(rename = "createdAt", skip_deserializing)]
    #[serde(with = "bson::serde_helpers::chrono_datetime_as_bson_datetime")]
    pub created_at: chrono::DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct LottoDraw {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,
    pub numbers: Vec<i32>,
    pub year: i32,
    #[serde(rename = "weekNumber")]
    pub week_number: u32,
    #[serde(with = "bson::serde_helpers::chrono_datetime_as_bson_datetime")]
    pub date: chrono::DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct LottoGuess {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,
    #[serde(rename = "dcId")]
    pub dc_id: u64,
    #[serde(rename = "dcUsername", skip_serializing_if = "Option::is_none")]
    pub dc_username: Option<String>,
    pub numbers: Vec<i32>,
    #[serde(skip_deserializing)]
    pub year: i32,
    #[serde(rename = "weekNumber", skip_deserializing)]
    pub week_number: u32,
    #[serde(rename = "matchedCount", skip_serializing_if = "Option::is_none")]
    pub matched_count: Option<i32>,
    #[serde(rename = "isMatched", skip_serializing_if = "Option::is_none")]
    pub is_any_matched: Option<bool>,
    pub points: Option<i32>,
    #[serde(rename = "dmSent", skip_serializing_if = "Option::is_none")]
    pub dm_sent: Option<bool>,
    #[serde(rename = "createdAt", skip_deserializing)]
    #[serde(with = "bson::serde_helpers::chrono_datetime_as_bson_datetime")]
    pub created_at: chrono::DateTime<Utc>,
    #[serde(rename = "updatedAt")]
    #[serde(with = "bson::serde_helpers::chrono_datetime_as_bson_datetime")]
    pub updated_at: chrono::DateTime<Utc>,
}
