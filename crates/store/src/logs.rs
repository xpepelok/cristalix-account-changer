use sqlx::{Row, SqliteConnection};

use crate::error::Result;
use crate::model::{LogLine, LogSessionInfo, LogStats, LogSummary};
use crate::store::{Store, now};

pub const MAX_LOG_LINES: i64 = 4000;

impl Store {
    pub async fn begin_log_session(&self, uuid: &str, name: &str) -> Result<i64> {
        let mut tx = self.pool.begin().await?;
        sqlx::query(
            "INSERT INTO log_clients (uuid, name) VALUES (?1, ?2) \
             ON CONFLICT (uuid) DO UPDATE SET name = ?2",
        )
        .bind(uuid)
        .bind(name)
        .execute(&mut *tx)
        .await?;
        deactivate_latest(&mut tx, uuid).await?;
        let id = sqlx::query("INSERT INTO log_sessions (uuid, started, active) VALUES (?1, ?2, 1)")
            .bind(uuid)
            .bind(now())
            .execute(&mut *tx)
            .await?
            .last_insert_rowid();
        tx.commit().await?;
        Ok(id)
    }

    pub async fn append_log_line(&self, session_id: i64, text: &str) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        let next: i64 =
            sqlx::query("SELECT COALESCE(MAX(seq), -1) + 1 FROM log_lines WHERE session_id = ?1")
                .bind(session_id)
                .fetch_one(&mut *tx)
                .await?
                .try_get(0)?;
        sqlx::query("INSERT INTO log_lines (session_id, seq, at, text) VALUES (?1, ?2, ?3, ?4)")
            .bind(session_id)
            .bind(next)
            .bind(now())
            .bind(text)
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM log_lines WHERE session_id = ?1 AND seq <= ?2 - ?3")
            .bind(session_id)
            .bind(next)
            .bind(MAX_LOG_LINES)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn append_log_lines(&self, session_id: i64, lines: &[String]) -> Result<()> {
        if lines.is_empty() {
            return Ok(());
        }
        let mut tx = self.pool.begin().await?;
        let mut next: i64 =
            sqlx::query("SELECT COALESCE(MAX(seq), -1) + 1 FROM log_lines WHERE session_id = ?1")
                .bind(session_id)
                .fetch_one(&mut *tx)
                .await?
                .try_get(0)?;
        let at = now();
        for text in lines {
            sqlx::query(
                "INSERT INTO log_lines (session_id, seq, at, text) VALUES (?1, ?2, ?3, ?4)",
            )
            .bind(session_id)
            .bind(next)
            .bind(at)
            .bind(text)
            .execute(&mut *tx)
            .await?;
            next += 1;
        }
        sqlx::query("DELETE FROM log_lines WHERE session_id = ?1 AND seq < ?2 - ?3")
            .bind(session_id)
            .bind(next)
            .bind(MAX_LOG_LINES)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn finish_log_session(&self, session_id: i64) -> Result<()> {
        let ended = last_line_at(&self.pool, session_id).await?;
        sqlx::query("UPDATE log_sessions SET active = 0, ended = ?2 WHERE id = ?1 AND active = 1")
            .bind(session_id)
            .bind(ended)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn log_unsupported(&self, uuid: &str, name: &str, message: &str) -> Result<()> {
        let session = self.begin_log_session(uuid, name).await?;
        self.append_log_line(session, message).await?;
        self.finish_log_session(session).await
    }

    pub async fn log_summaries(&self) -> Result<Vec<LogSummary>> {
        let rows = sqlx::query(
            "SELECT c.uuid AS uuid, c.name AS name, s.active AS active, \
             (SELECT COUNT(*) FROM log_lines l WHERE l.session_id = s.id) AS lines \
             FROM log_clients c \
             JOIN log_sessions s ON s.id = (SELECT id FROM log_sessions WHERE uuid = c.uuid ORDER BY id DESC LIMIT 1)",
        )
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter()
            .map(|row| {
                Ok(LogSummary {
                    uuid: row.try_get("uuid")?,
                    name: row.try_get("name")?,
                    lines: row.try_get("lines")?,
                    active: row.try_get::<i64, _>("active")? != 0,
                })
            })
            .collect()
    }

    pub async fn log_sessions(&self, uuid: &str) -> Result<Vec<LogSessionInfo>> {
        let rows = sqlx::query(
            "SELECT s.id AS id, s.started AS started, s.ended AS ended, s.active AS active, \
             (SELECT COUNT(*) FROM log_lines l WHERE l.session_id = s.id) AS lines \
             FROM log_sessions s WHERE s.uuid = ?1 ORDER BY s.id DESC",
        )
        .bind(uuid)
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter()
            .map(|row| {
                Ok(LogSessionInfo {
                    id: row.try_get("id")?,
                    started: row.try_get("started")?,
                    ended: row.try_get("ended")?,
                    active: row.try_get::<i64, _>("active")? != 0,
                    lines: row.try_get("lines")?,
                })
            })
            .collect()
    }

    pub async fn log_lines(
        &self,
        uuid: &str,
        session_id: Option<i64>,
    ) -> Result<Option<Vec<LogLine>>> {
        let resolved = match session_id {
            Some(id) => id,
            None => {
                let row = sqlx::query(
                    "SELECT id FROM log_sessions WHERE uuid = ?1 ORDER BY id DESC LIMIT 1",
                )
                .bind(uuid)
                .fetch_optional(&self.pool)
                .await?;
                match row {
                    Some(row) => row.try_get("id")?,
                    None => return Ok(None),
                }
            }
        };
        let exists = sqlx::query("SELECT 1 FROM log_sessions WHERE id = ?1 AND uuid = ?2")
            .bind(resolved)
            .bind(uuid)
            .fetch_optional(&self.pool)
            .await?
            .is_some();
        if !exists {
            return Ok(None);
        }
        let rows = sqlx::query("SELECT at, text FROM log_lines WHERE session_id = ?1 ORDER BY seq")
            .bind(resolved)
            .fetch_all(&self.pool)
            .await?;
        let lines = rows
            .into_iter()
            .map(|row| {
                Ok(LogLine {
                    at: row.try_get("at")?,
                    text: row.try_get("text")?,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(Some(lines))
    }

    pub async fn clear_log(&self, uuid: &str) -> Result<()> {
        sqlx::query("DELETE FROM log_clients WHERE uuid = ?1")
            .bind(uuid)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn clear_all_logs(&self) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("DELETE FROM log_sessions WHERE active = 0")
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM log_clients WHERE uuid NOT IN (SELECT uuid FROM log_sessions)")
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn delete_log_session(&self, uuid: &str, session_id: i64) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("DELETE FROM log_sessions WHERE id = ?1 AND uuid = ?2 AND active = 0")
            .bind(session_id)
            .bind(uuid)
            .execute(&mut *tx)
            .await?;
        sqlx::query(
            "DELETE FROM log_clients WHERE uuid = ?1 AND uuid NOT IN (SELECT uuid FROM log_sessions)",
        )
        .bind(uuid)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn log_size_all(&self) -> Result<i64> {
        let size: i64 = sqlx::query("SELECT COALESCE(SUM(LENGTH(text) + 1), 0) FROM log_lines")
            .fetch_one(&self.pool)
            .await?
            .try_get(0)?;
        Ok(size)
    }

    pub async fn log_stats(&self) -> Result<LogStats> {
        let mut stats = LogStats::default();
        let now = now();
        stats.total_accounts = scalar(&self.pool, "SELECT COUNT(*) FROM log_clients").await?;
        let rows = sqlx::query(
            "SELECT s.uuid AS uuid, c.name AS name, s.started AS started, s.ended AS ended, \
             s.active AS active, \
             (SELECT COUNT(*) FROM log_lines l WHERE l.session_id = s.id) AS lines \
             FROM log_sessions s JOIN log_clients c ON c.uuid = s.uuid",
        )
        .fetch_all(&self.pool)
        .await?;
        let mut counts: std::collections::HashMap<String, (String, i64)> =
            std::collections::HashMap::new();
        for row in rows {
            let uuid: String = row.try_get("uuid")?;
            let name: String = row.try_get("name")?;
            let started: i64 = row.try_get("started")?;
            let ended: i64 = row.try_get("ended")?;
            let active = row.try_get::<i64, _>("active")? != 0;
            let lines: i64 = row.try_get("lines")?;
            stats.total_sessions += 1;
            stats.total_lines += lines;
            let end = if active {
                stats.active_now += 1;
                now
            } else {
                ended
            };
            let duration = (end - started).max(0);
            stats.total_duration += duration;
            if duration > stats.longest_duration {
                stats.longest_duration = duration;
                stats.longest_name = name.clone();
                stats.longest_uuid = uuid.clone();
            }
            let entry = counts.entry(uuid).or_insert((name, 0));
            entry.1 += 1;
        }
        for (uuid, (name, count)) in counts {
            if count > stats.most_launched_count {
                stats.most_launched_count = count;
                stats.most_launched_name = name;
                stats.most_launched_uuid = uuid;
            }
        }
        if stats.total_sessions > 0 {
            stats.avg_duration = stats.total_duration / stats.total_sessions;
        }
        Ok(stats)
    }
}

async fn deactivate_latest(tx: &mut SqliteConnection, uuid: &str) -> Result<()> {
    let row = sqlx::query(
        "SELECT id FROM log_sessions WHERE uuid = ?1 AND active = 1 ORDER BY id DESC LIMIT 1",
    )
    .bind(uuid)
    .fetch_optional(&mut *tx)
    .await?;
    let Some(row) = row else {
        return Ok(());
    };
    let id: i64 = row.try_get("id")?;
    let ended: i64 =
        sqlx::query("SELECT COALESCE(MAX(at), ?2) FROM log_lines WHERE session_id = ?1")
            .bind(id)
            .bind(now())
            .fetch_one(&mut *tx)
            .await?
            .try_get(0)?;
    sqlx::query("UPDATE log_sessions SET active = 0, ended = ?2 WHERE id = ?1")
        .bind(id)
        .bind(ended)
        .execute(&mut *tx)
        .await?;
    Ok(())
}

async fn last_line_at(pool: &sqlx::SqlitePool, session_id: i64) -> Result<i64> {
    let value: i64 =
        sqlx::query("SELECT COALESCE(MAX(at), ?2) FROM log_lines WHERE session_id = ?1")
            .bind(session_id)
            .bind(now())
            .fetch_one(pool)
            .await?
            .try_get(0)?;
    Ok(value)
}

async fn scalar(pool: &sqlx::SqlitePool, query: &'static str) -> Result<i64> {
    Ok(sqlx::query(query).fetch_one(pool).await?.try_get(0)?)
}
