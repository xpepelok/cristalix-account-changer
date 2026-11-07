use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use sqlx::Row;

use crate::error::Result;
use crate::model::Group;
use crate::store::Store;

static GROUP_COUNTER: AtomicU64 = AtomicU64::new(0);

fn new_group_id() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_nanos())
        .unwrap_or_default();
    let counter = GROUP_COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("g{nanos:032x}{counter:04x}")
}

fn clamp_name(name: &str) -> String {
    name.trim().chars().take(Store::max_group_name()).collect()
}

impl Store {
    pub(crate) async fn load_groups(&self) -> Result<Vec<Group>> {
        let mut groups = Vec::new();
        for row in sqlx::query(
            "SELECT id, name, profile, pinned, sort_order FROM account_groups ORDER BY position",
        )
        .fetch_all(&self.pool)
        .await?
        {
            let id: String = row.try_get("id")?;
            let members = sqlx::query(
                "SELECT account_uuid FROM group_members WHERE group_id = ?1 ORDER BY position",
            )
            .bind(&id)
            .fetch_all(&self.pool)
            .await?
            .into_iter()
            .map(|member| member.try_get::<String, _>("account_uuid"))
            .collect::<std::result::Result<Vec<_>, _>>()?;
            groups.push(Group {
                id,
                name: row.try_get("name")?,
                profile: row.try_get("profile")?,
                pinned: row.try_get::<i64, _>("pinned")? != 0,
                order: row.try_get("sort_order")?,
                members,
            });
        }
        Ok(groups)
    }

    async fn persist_group(&self, group: &Group, position: i64) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        sqlx::query(
            "INSERT INTO account_groups (id, name, profile, pinned, sort_order, position) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6) \
             ON CONFLICT (id) DO UPDATE SET name = ?2, profile = ?3, pinned = ?4, sort_order = ?5, \
             position = ?6",
        )
        .bind(&group.id)
        .bind(&group.name)
        .bind(&group.profile)
        .bind(i64::from(group.pinned))
        .bind(group.order)
        .bind(position)
        .execute(&mut *tx)
        .await?;
        sqlx::query("DELETE FROM group_members WHERE group_id = ?1")
            .bind(&group.id)
            .execute(&mut *tx)
            .await?;
        for (index, member) in group.members.iter().enumerate() {
            sqlx::query(
                "INSERT INTO group_members (group_id, account_uuid, position) VALUES (?1, ?2, ?3)",
            )
            .bind(&group.id)
            .bind(member)
            .bind(index as i64)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    async fn persist_positions(&self) -> Result<()> {
        let groups = self.read_groups(<[Group]>::to_vec);
        let mut tx = self.pool.begin().await?;
        for (index, group) in groups.iter().enumerate() {
            sqlx::query("UPDATE account_groups SET sort_order = ?1, position = ?2 WHERE id = ?3")
                .bind(group.order)
                .bind(index as i64)
                .bind(&group.id)
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    fn group_position(&self, id: &str) -> i64 {
        self.read_groups(|groups| {
            groups
                .iter()
                .position(|group| group.id == id)
                .map_or(0, |index| index as i64)
        })
    }

    pub fn sorted_groups(&self) -> Vec<Group> {
        self.read_groups(|groups| {
            let mut out = groups.to_vec();
            out.sort_by(|a, b| b.pinned.cmp(&a.pinned).then(a.order.cmp(&b.order)));
            out
        })
    }

    pub async fn create_group(&self, name: &str) -> Result<Option<Group>> {
        let name = clamp_name(name);
        if name.is_empty() {
            return Ok(None);
        }
        let group = self.with_groups(|groups| {
            let max_order = groups.iter().fold(0, |acc, group| acc.max(group.order + 1));
            let group = Group {
                id: new_group_id(),
                name,
                members: Vec::new(),
                profile: String::new(),
                pinned: false,
                order: max_order,
            };
            groups.push(group.clone());
            group
        });
        let position = self.group_position(&group.id);
        self.persist_group(&group, position).await?;
        self.bump_revision();
        Ok(Some(group))
    }

    pub async fn delete_group(&self, id: &str) -> Result<()> {
        let removed = self.with_groups(|groups| {
            let before = groups.len();
            groups.retain(|group| group.id != id);
            groups.len() != before
        });
        if !removed {
            return Ok(());
        }
        sqlx::query("DELETE FROM account_groups WHERE id = ?1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        self.persist_positions().await?;
        self.bump_revision();
        Ok(())
    }

    pub async fn rename_group(&self, id: &str, name: &str) -> Result<bool> {
        let name = clamp_name(name);
        if name.is_empty() {
            return Ok(false);
        }
        self.mutate_group(id, |group| group.name = name.clone())
            .await
    }

    pub async fn set_group_pinned(&self, id: &str, pinned: bool) -> Result<bool> {
        self.mutate_group(id, |group| group.pinned = pinned).await
    }

    pub async fn set_group_profile(&self, id: &str, profile: &str) -> Result<bool> {
        self.mutate_group(id, |group| group.profile = profile.to_owned())
            .await
    }

    async fn mutate_group(&self, id: &str, mutate: impl FnOnce(&mut Group)) -> Result<bool> {
        let group = self.with_groups(|groups| {
            let group = groups.iter_mut().find(|group| group.id == id)?;
            mutate(group);
            Some(group.clone())
        });
        let Some(group) = group else {
            return Ok(false);
        };
        let position = self.group_position(id);
        self.persist_group(&group, position).await?;
        self.bump_revision();
        Ok(true)
    }

    pub async fn reorder_groups(&self, ids: &[String]) -> Result<()> {
        self.with_groups(|groups| {
            for group in groups.iter_mut() {
                if let Some(index) = ids.iter().position(|id| id == &group.id) {
                    group.order = index as i64;
                }
            }
        });
        self.persist_positions().await?;
        self.bump_revision();
        Ok(())
    }

    pub async fn add_group_member(&self, id: &str, uuid: &str) -> Result<bool> {
        if !self.account_exists(uuid) {
            return Ok(false);
        }
        let group = self.with_groups(|groups| {
            let group = groups.iter_mut().find(|group| group.id == id)?;
            if group.members.iter().any(|member| member == uuid) {
                return None;
            }
            group.members.push(uuid.to_owned());
            Some(group.clone())
        });
        let Some(group) = group else {
            return Ok(false);
        };
        let position = self.group_position(id);
        self.persist_group(&group, position).await?;
        self.bump_revision();
        Ok(true)
    }

    pub async fn remove_group_member(&self, id: &str, uuid: &str) -> Result<bool> {
        let group = self.with_groups(|groups| {
            let group = groups.iter_mut().find(|group| group.id == id)?;
            let before = group.members.len();
            group.members.retain(|member| member != uuid);
            if group.members.len() == before {
                return None;
            }
            Some(group.clone())
        });
        let Some(group) = group else {
            return Ok(false);
        };
        let position = self.group_position(id);
        self.persist_group(&group, position).await?;
        self.bump_revision();
        Ok(true)
    }

    pub fn group_members(&self, id: &str) -> Option<(Vec<String>, String)> {
        self.read_groups(|groups| {
            let group = groups.iter().find(|group| group.id == id)?;
            let members = group.members.clone();
            Some((members, group.profile.clone()))
        })
        .map(|(members, profile)| {
            let existing: Vec<String> = members
                .into_iter()
                .filter(|uuid| self.account_exists(uuid))
                .collect();
            (existing, profile)
        })
    }
}
