use sqlx::Row;

use crate::db::{pack_pids, unpack_pids};
use crate::error::Result;
use crate::model::GameLaunch;
use crate::store::Store;

impl Store {
    pub async fn load_launches(&self) -> Result<Vec<GameLaunch>> {
        let rows = sqlx::query(
            "SELECT uuid, launched_at, pid, seen_at, before_pids FROM game_launches ORDER BY position",
        )
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter()
            .map(|row| {
                Ok(GameLaunch {
                    uuid: row.try_get("uuid")?,
                    launched_at: row.try_get("launched_at")?,
                    pid: row.try_get::<i64, _>("pid")? as u32,
                    seen_at: row.try_get("seen_at")?,
                    before_pids: unpack_pids(&row.try_get::<Vec<u8>, _>("before_pids")?),
                })
            })
            .collect()
    }

    pub async fn save_launches(&self, launches: &[GameLaunch]) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("DELETE FROM game_launches")
            .execute(&mut *tx)
            .await?;
        for (index, launch) in launches.iter().enumerate() {
            sqlx::query(
                "INSERT INTO game_launches (uuid, position, launched_at, pid, seen_at, before_pids) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            )
            .bind(&launch.uuid)
            .bind(index as i64)
            .bind(launch.launched_at)
            .bind(i64::from(launch.pid))
            .bind(launch.seen_at)
            .bind(pack_pids(&launch.before_pids))
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(())
    }
}
