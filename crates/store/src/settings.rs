use sqlx::Row;

use crate::error::Result;
use crate::model::{LauncherKind, Settings, Theme};
use crate::store::Store;

impl Store {
    pub(crate) async fn load_settings(&self) -> Result<Settings> {
        let row = sqlx::query("SELECT * FROM settings WHERE id = 1")
            .fetch_one(&self.pool)
            .await?;
        let launcher =
            LauncherKind::parse(row.try_get::<String, _>("launcher")?.as_str()).unwrap_or_default();
        Ok(Settings {
            launcher,
            custom_launcher: row.try_get("custom_launcher")?,
            auto_play: row.try_get::<i64, _>("auto_play")? != 0,
            stats: row.try_get::<i64, _>("stats")? != 0,
            aggressive: row.try_get::<i64, _>("aggressive")? != 0,
            window_title: row.try_get::<i64, _>("window_title")? != 0,
            background: row.try_get("background")?,
            background_dim: row.try_get::<i64, _>("background_dim")? as u8,
            background_blur: row.try_get::<i64, _>("background_blur")? as u8,
            theme: Theme::parse(row.try_get::<String, _>("theme")?.as_str()).unwrap_or_default(),
            sound: row.try_get::<i64, _>("sound")? != 0,
        })
    }

    async fn persist_settings(&self) -> Result<()> {
        let settings = self.settings();
        sqlx::query(
            "UPDATE settings SET launcher = ?1, custom_launcher = ?2, auto_play = ?3, stats = ?4, \
             aggressive = ?5, window_title = ?6, background = ?7, background_dim = ?8, \
             background_blur = ?9, theme = ?10, sound = ?11 WHERE id = 1",
        )
        .bind(settings.launcher.as_str())
        .bind(&settings.custom_launcher)
        .bind(i64::from(settings.auto_play))
        .bind(i64::from(settings.stats))
        .bind(i64::from(settings.aggressive))
        .bind(i64::from(settings.window_title))
        .bind(settings.background.as_deref())
        .bind(i64::from(settings.background_dim))
        .bind(i64::from(settings.background_blur))
        .bind(settings.theme.as_str())
        .bind(i64::from(settings.sound))
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn mutate_settings(&self, mutate: impl FnOnce(&mut Settings)) -> Result<()> {
        mutate(&mut self.settings_lock().write());
        self.persist_settings().await?;
        self.bump_revision();
        Ok(())
    }

    pub fn launcher(&self) -> LauncherKind {
        self.settings().launcher
    }

    pub fn custom_launcher(&self) -> String {
        self.settings().custom_launcher
    }

    pub async fn set_launcher(&self, launcher: LauncherKind) -> Result<bool> {
        if launcher == LauncherKind::Custom && self.settings().custom_launcher.is_empty() {
            return Ok(false);
        }
        self.mutate_settings(|settings| settings.launcher = launcher)
            .await?;
        Ok(true)
    }

    pub async fn set_custom_launcher(&self, path: &str) -> Result<bool> {
        if path.is_empty() {
            return Ok(false);
        }
        self.mutate_settings(|settings| {
            settings.custom_launcher = path.to_owned();
            settings.launcher = LauncherKind::Custom;
        })
        .await?;
        Ok(true)
    }

    pub fn auto_play(&self) -> bool {
        self.settings().auto_play
    }

    pub async fn set_auto_play(&self, value: bool) -> Result<()> {
        self.mutate_settings(|settings| settings.auto_play = value)
            .await
    }

    pub fn stats(&self) -> bool {
        self.settings().stats
    }

    pub async fn set_stats(&self, value: bool) -> Result<()> {
        self.mutate_settings(|settings| settings.stats = value)
            .await
    }

    pub fn aggressive_launch(&self) -> bool {
        self.settings().aggressive
    }

    pub async fn set_aggressive_launch(&self, value: bool) -> Result<()> {
        self.mutate_settings(|settings| settings.aggressive = value)
            .await
    }

    pub fn window_title(&self) -> bool {
        self.settings().window_title
    }

    pub async fn set_window_title(&self, value: bool) -> Result<()> {
        self.mutate_settings(|settings| settings.window_title = value)
            .await
    }

    pub fn background(&self) -> Option<String> {
        self.settings().background
    }

    pub async fn set_background(&self, background: Option<String>) -> Result<()> {
        self.mutate_settings(|settings| settings.background = background)
            .await
    }

    pub fn background_dim(&self) -> u8 {
        self.settings().background_dim
    }

    pub async fn set_background_dim(&self, percent: u8) -> Result<()> {
        let percent = percent.min(100);
        self.mutate_settings(|settings| settings.background_dim = percent)
            .await
    }

    pub fn background_blur(&self) -> u8 {
        self.settings().background_blur
    }

    pub async fn set_background_blur(&self, pixels: u8) -> Result<()> {
        let pixels = pixels.min(40);
        self.mutate_settings(|settings| settings.background_blur = pixels)
            .await
    }

    pub fn theme(&self) -> Theme {
        self.settings().theme
    }

    pub async fn set_theme(&self, theme: Theme) -> Result<()> {
        self.mutate_settings(|settings| settings.theme = theme)
            .await
    }

    pub fn sound(&self) -> bool {
        self.settings().sound
    }

    pub async fn set_sound(&self, value: bool) -> Result<()> {
        self.mutate_settings(|settings| settings.sound = value)
            .await
    }
}
