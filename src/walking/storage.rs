use crate::{
    error::{Error, Result, invalid},
    storage::Store,
};

use super::WalkingObservation;

impl Store {
    pub(super) async fn save_walking_observation(
        &self,
        user: &str,
        input: &WalkingObservation,
    ) -> Result<(f64, u32)> {
        let mut transaction = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let owner: Option<String> =
            sqlx::query_scalar("SELECT id FROM routines WHERE user_id = ? AND id = ?")
                .bind(user)
                .bind(&input.routine_id)
                .fetch_optional(&mut *transaction)
                .await?;
        owner.ok_or(Error::NotFound)?;
        let origin = serde_json::to_string(&input.origin)?;
        let destination = serde_json::to_string(&input.destination)?;
        let previous: Option<(String, String, f64, f64, String)> = sqlx::query_as(
            "SELECT origin, destination, distance_meters, elapsed_minutes, distance_source
             FROM walking_observations WHERE user_id = ? AND routine_id = ? AND observation_id = ?",
        )
        .bind(user)
        .bind(&input.routine_id)
        .bind(&input.observation_id)
        .fetch_optional(&mut *transaction)
        .await?;
        if let Some((old_origin, old_destination, distance, elapsed, source)) = previous {
            if old_origin != origin
                || old_destination != destination
                || distance != input.distance_meters
                || elapsed != input.elapsed_minutes
                || source != input.distance_source.as_str()
            {
                return Err(invalid("observation_id already has different walking data"));
            }
        } else {
            sqlx::query(
                "INSERT INTO walking_observations
                 (user_id, routine_id, observation_id, origin, destination, distance_meters, elapsed_minutes, distance_source)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(user)
            .bind(&input.routine_id)
            .bind(&input.observation_id)
            .bind(origin)
            .bind(destination)
            .bind(input.distance_meters)
            .bind(input.elapsed_minutes)
            .bind(input.distance_source.as_str())
            .execute(&mut *transaction)
            .await?;
        }
        let (distance, elapsed, count): (f64, f64, i64) = sqlx::query_as(
            "SELECT SUM(distance_meters), SUM(elapsed_minutes), COUNT(*)
             FROM walking_observations WHERE user_id = ? AND routine_id = ?",
        )
        .bind(user)
        .bind(&input.routine_id)
        .fetch_one(&mut *transaction)
        .await?;
        let speed = distance * 0.06 / elapsed;
        if !speed.is_finite() || !(0.5..=12.0).contains(&speed) {
            return Err(invalid("aggregate walking speed must be 0.5..12 km/h"));
        }
        let count = u32::try_from(count).map_err(|_| invalid("too many walking observations"))?;
        sqlx::query("UPDATE routines SET body = json_set(body, '$.walking_speed_kmh', ?) WHERE user_id = ? AND id = ?")
            .bind(speed)
            .bind(user)
            .bind(&input.routine_id)
            .execute(&mut *transaction)
            .await?;
        transaction.commit().await?;
        Ok((speed, count))
    }
}
