use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::Serialize;
use sqlx::Row;

use crate::{
    error::{Result, invalid},
    routines::{date, text},
    storage::Store,
};

#[derive(Debug, Serialize, JsonSchema)]
pub struct Alert {
    pub id: i64,
    pub routine_id: String,
    pub kind: String,
    pub created_at: String,
    pub message: String,
}

pub struct AlertDraft {
    pub routine_id: String,
    pub date: String,
    pub kind: String,
    pub now: DateTime<Utc>,
    pub message: String,
    pub min_interval_minutes: u32,
}

impl Store {
    pub async fn enqueue_alert(&self, user: &str, input: &AlertDraft) -> Result<bool> {
        date(&input.date)?;
        text(&input.kind)?;
        if input.message.trim().is_empty()
            || input.message.len() > 4000
            || !(1..=1440).contains(&input.min_interval_minutes)
        {
            return Err(invalid("alert message or interval out of range"));
        }
        self.get(user, &input.routine_id).await?;
        let cutoff = input.now.timestamp_millis() - i64::from(input.min_interval_minutes) * 60_000;
        // A single write statement serializes the spacing check across processes too.
        let result = sqlx::query(
            "INSERT INTO alerts(user_id, routine_id, date, kind, created_at, timestamp_ms, message)
             SELECT ?, ?, ?, ?, ?, ?, ?
             WHERE NOT EXISTS (SELECT 1 FROM alerts WHERE user_id = ? AND routine_id = ? AND timestamp_ms > ?)
             ON CONFLICT(user_id, routine_id, date, kind) DO NOTHING"
        ).bind(user).bind(&input.routine_id).bind(&input.date).bind(&input.kind)
            .bind(input.now.to_rfc3339()).bind(input.now.timestamp_millis()).bind(&input.message)
            .bind(user).bind(&input.routine_id).bind(cutoff).execute(&self.pool).await?;
        Ok(result.rows_affected() == 1)
    }

    pub async fn alerts(&self, user: &str) -> Result<Vec<Alert>> {
        let rows = sqlx::query("SELECT id, routine_id, kind, created_at, message FROM alerts WHERE user_id = ? ORDER BY id DESC")
            .bind(user).fetch_all(&self.pool).await?;
        rows.into_iter()
            .map(|row| {
                Ok(Alert {
                    id: row.try_get("id")?,
                    routine_id: row.try_get("routine_id")?,
                    kind: row.try_get("kind")?,
                    created_at: row.try_get("created_at")?,
                    message: row.try_get("message")?,
                })
            })
            .collect()
    }
}
