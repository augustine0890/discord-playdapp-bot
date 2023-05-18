#![allow(dead_code)]
use chrono::{Datelike, Utc};

pub fn is_wed() -> bool {
    let now = Utc::now();
    now.weekday() == chrono::Weekday::Wed
}
