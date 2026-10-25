use std::{str::FromStr, time::Duration};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sqlx::{
    ConnectOptions, SqlitePool,
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
};

use crate::{
    error::{Error, Result, confirmed, invalid},
    routines::{Routine, date, text},
};

#[derive(Clone)]
pub struct Store {
    pub(crate) pool: SqlitePool,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DayStatus {
    pub routine_id: String,
    pub date: String,
    pub enabled: bool,
    pub confirmed: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Feedback {
    pub routine_id: String,
    pub route_id: String,
    pub actual_minutes: u32,
    pub walking_factor: Option<f64>,
    pub confirmed: bool,
}

impl Store {
    pub async fn open(url: &str) -> Result<Self> {
        let options = SqliteConnectOptions::from_str(url)?
            .create_if_missing(true)
            .foreign_keys(true)
            .busy_timeout(Duration::from_secs(5))
            .disable_statement_logging();
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await?;
        sqlx::raw_sql(
            "CREATE TABLE IF NOT EXISTS routines (
                user_id TEXT NOT NULL, id TEXT NOT NULL, body TEXT NOT NULL,
                PRIMARY KEY (user_id, id));
             CREATE TABLE IF NOT EXISTS day_status (
                user_id TEXT NOT NULL, routine_id TEXT NOT NULL, date TEXT NOT NULL,
                enabled INTEGER NOT NULL, PRIMARY KEY (user_id, routine_id, date),
                FOREIGN KEY (user_id, routine_id) REFERENCES routines(user_id, id));
             CREATE TABLE IF NOT EXISTS feedback (
                id INTEGER PRIMARY KEY, user_id TEXT NOT NULL, routine_id TEXT NOT NULL,
                body TEXT NOT NULL, created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY (user_id, routine_id) REFERENCES routines(user_id, id));
             CREATE TABLE IF NOT EXISTS alerts (
                id INTEGER PRIMARY KEY, user_id TEXT NOT NULL, routine_id TEXT NOT NULL,
                date TEXT NOT NULL, kind TEXT NOT NULL, created_at TEXT NOT NULL,
                timestamp_ms INTEGER NOT NULL, message TEXT NOT NULL,
                UNIQUE (user_id, routine_id, date, kind),
                FOREIGN KEY (user_id, routine_id) REFERENCES routines(user_id, id));
             CREATE INDEX IF NOT EXISTS alerts_spacing ON alerts(user_id, routine_id, timestamp_ms);"
        ).execute(&pool).await?;
        Ok(Self { pool })
    }

    pub async fn list(&self, user: &str) -> Result<Vec<Routine>> {
        let rows: Vec<String> =
            sqlx::query_scalar("SELECT body FROM routines WHERE user_id = ? ORDER BY id")
                .bind(user)
                .fetch_all(&self.pool)
                .await?;
        rows.into_iter()
            .map(|body| Ok(serde_json::from_str(&body)?))
            .collect()
    }

    pub async fn get(&self, user: &str, id: &str) -> Result<Routine> {
        let body: String =
            sqlx::query_scalar("SELECT body FROM routines WHERE user_id = ? AND id = ?")
                .bind(user)
                .bind(id)
                .fetch_optional(&self.pool)
                .await?
                .ok_or(Error::NotFound)?;
        Ok(serde_json::from_str(&body)?)
    }

    pub async fn upsert(&self, user: &str, routine: &Routine) -> Result<()> {
        text(user)?;
        routine.validate()?;
        sqlx::query("INSERT INTO routines(user_id, id, body) VALUES (?, ?, ?) ON CONFLICT(user_id, id) DO UPDATE SET body = excluded.body")
            .bind(user).bind(&routine.id).bind(serde_json::to_string(routine)?).execute(&self.pool).await?;
        Ok(())
    }

    pub async fn set_day(&self, user: &str, input: &DayStatus) -> Result<()> {
        confirmed(input.confirmed)?;
        date(&input.date)?;
        self.get(user, &input.routine_id).await?;
        sqlx::query("INSERT INTO day_status(user_id, routine_id, date, enabled) VALUES (?, ?, ?, ?) ON CONFLICT(user_id, routine_id, date) DO UPDATE SET enabled = excluded.enabled")
            .bind(user).bind(&input.routine_id).bind(&input.date).bind(input.enabled).execute(&self.pool).await?;
        Ok(())
    }

    pub async fn day_enabled(&self, user: &str, id: &str, day: &str) -> Result<Option<bool>> {
        date(day)?;
        self.get(user, id).await?;
        Ok(sqlx::query_scalar(
            "SELECT enabled FROM day_status WHERE user_id = ? AND routine_id = ? AND date = ?",
        )
        .bind(user)
        .bind(id)
        .bind(day)
        .fetch_optional(&self.pool)
        .await?)
    }

    pub async fn feedback(&self, user: &str, input: &Feedback) -> Result<()> {
        confirmed(input.confirmed)?;
        text(&input.route_id)?;
        if !(1..=1440).contains(&input.actual_minutes)
            || input
                .walking_factor
                .is_some_and(|factor| !factor.is_finite() || !(0.5..=3.0).contains(&factor))
        {
            return Err(invalid("feedback duration or walking factor out of range"));
        }
        self.get(user, &input.routine_id).await?;
        let mut transaction = self.pool.begin().await?;
        sqlx::query("INSERT INTO feedback(user_id, routine_id, body) VALUES (?, ?, ?)")
            .bind(user)
            .bind(&input.routine_id)
            .bind(serde_json::to_string(input)?)
            .execute(&mut *transaction)
            .await?;
        if let Some(factor) = input.walking_factor {
            sqlx::query("UPDATE routines SET body = json_set(body, '$.walking_factor', ?) WHERE user_id = ? AND id = ?")
                .bind(factor).bind(user).bind(&input.routine_id).execute(&mut *transaction).await?;
        }
        transaction.commit().await?;
        Ok(())
    }

    pub async fn all_routines(&self) -> Result<Vec<(String, Routine)>> {
        let rows: Vec<(String, String)> =
            sqlx::query_as("SELECT user_id, body FROM routines ORDER BY user_id, id")
                .fetch_all(&self.pool)
                .await?;
        rows.into_iter()
            .map(|(user, body)| Ok((user, serde_json::from_str(&body)?)))
            .collect()
    }

    pub async fn health(&self) -> Result<()> {
        sqlx::query("SELECT 1").execute(&self.pool).await?;
        Ok(())
    }
}
