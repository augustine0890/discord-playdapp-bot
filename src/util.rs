use chrono::Utc;

pub fn is_wed() -> bool {
    let now = Utc::now();
    now.weekday() == chrono::Weekday::Wed
}

pub fn is_thu() -> bool {
    let now = Utc::now();
    now.weekday() == chrono::Weekday::Thu
}
