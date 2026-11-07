use std::time::{Duration, SystemTime, UNIX_EPOCH};

use ac_api::{ClientToggle, LaunchSettings};
use ac_launcher::Presence;

pub(crate) fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_secs() as i64)
        .unwrap_or_default()
}

pub(crate) fn to_unix(time: SystemTime) -> i64 {
    time.duration_since(UNIX_EPOCH)
        .map(|value| value.as_secs() as i64)
        .unwrap_or_default()
}

pub(crate) fn from_unix(seconds: i64) -> SystemTime {
    UNIX_EPOCH + Duration::from_secs(seconds.max(0).unsigned_abs())
}

pub(crate) fn is_expired(expires: i64, now: i64) -> bool {
    expires > 0 && expires < now
}

fn unsigned(value: i64) -> u32 {
    u32::try_from(value.max(0)).unwrap_or(u32::MAX)
}

fn toggle(value: i64) -> ClientToggle {
    ClientToggle::from(u8::try_from(value).unwrap_or(0))
}

pub(crate) fn launcher_kind(kind: ac_store::LauncherKind) -> ac_api::LauncherKind {
    match kind {
        ac_store::LauncherKind::Normal => ac_api::LauncherKind::Normal,
        ac_store::LauncherKind::Jar => ac_api::LauncherKind::Jar,
        ac_store::LauncherKind::New => ac_api::LauncherKind::New,
        ac_store::LauncherKind::Custom => ac_api::LauncherKind::Custom,
    }
}

pub(crate) fn store_launcher_kind(kind: ac_api::LauncherKind) -> ac_store::LauncherKind {
    match kind {
        ac_api::LauncherKind::Normal => ac_store::LauncherKind::Normal,
        ac_api::LauncherKind::Jar => ac_store::LauncherKind::Jar,
        ac_api::LauncherKind::New => ac_store::LauncherKind::New,
        ac_api::LauncherKind::Custom => ac_store::LauncherKind::Custom,
    }
}

pub(crate) fn theme(theme: ac_store::Theme) -> ac_api::Theme {
    match theme {
        ac_store::Theme::Dark => ac_api::Theme::Dark,
        ac_store::Theme::Light => ac_api::Theme::Light,
    }
}

pub(crate) fn store_theme(theme: ac_api::Theme) -> ac_store::Theme {
    match theme {
        ac_api::Theme::Dark => ac_store::Theme::Dark,
        ac_api::Theme::Light => ac_store::Theme::Light,
    }
}

pub(crate) fn launch_account(account: ac_store::Account) -> ac_launcher::Account {
    ac_launcher::Account {
        uuid: account.uuid,
        name: account.name,
        token: account.token,
        client: account.client,
        profile: account.profile,
        ram: account.ram,
        min_graphics: account.min_graphics,
        fullscreen: account.fullscreen,
        discord_rpc: account.discord_rpc,
        auto_enter: account.auto_enter,
        debug_mode: account.debug_mode,
        render_distance: account.render_distance,
        max_fps: account.max_fps,
        animations: account.animations,
        fast_render: account.fast_render,
        minimal: account.minimal,
        last_launched: account.last_launched,
    }
}

pub(crate) fn accounts(list: Vec<ac_store::Account>, presence: &Presence) -> Vec<ac_api::Account> {
    let now = unix_now();
    list.into_iter()
        .filter(|account| !account.name.is_empty() && !account.token.is_empty())
        .map(|account| {
            let running = presence.is_running(&account.uuid);
            ac_api::Account {
                launchable: true,
                expired: is_expired(account.expires, now),
                expires: account.expires,
                last_launched: account.last_launched,
                last_online: account.last_online,
                running,
                launching: !running && presence.launching.contains(&account.uuid),
                label: account.label,
                pinned: account.pinned,
                client: account.client,
                profile: account.profile,
                launch: LaunchSettings {
                    ram: unsigned(account.ram),
                    min_graphics: account.min_graphics,
                    fullscreen: account.fullscreen,
                    discord_rpc: account.discord_rpc,
                    auto_enter: account.auto_enter,
                    debug_mode: account.debug_mode,
                    render_distance: unsigned(account.render_distance),
                    max_fps: unsigned(account.max_fps),
                    animations: toggle(account.animations),
                    fast_render: toggle(account.fast_render),
                    minimal: account.minimal,
                },
                first_seen: account.first_seen,
                uuid: account.uuid,
                name: account.name,
            }
        })
        .collect()
}

pub(crate) fn launch_opts(settings: &LaunchSettings, ram: i64) -> ac_store::LaunchOpts {
    ac_store::LaunchOpts {
        ram,
        min_graphics: settings.min_graphics,
        fullscreen: settings.fullscreen,
        discord_rpc: settings.discord_rpc,
        auto_enter: settings.auto_enter,
        debug_mode: settings.debug_mode,
        render_distance: i64::from(settings.render_distance),
        max_fps: i64::from(settings.max_fps),
        animations: i64::from(u8::from(settings.animations)),
        fast_render: i64::from(u8::from(settings.fast_render)),
        minimal: settings.minimal,
    }
}

pub(crate) fn group(group: ac_store::Group) -> ac_api::Group {
    ac_api::Group {
        id: group.id,
        name: group.name,
        members: group.members,
        profile: group.profile,
        pinned: group.pinned,
        order: i32::try_from(group.order).unwrap_or(i32::MAX),
    }
}

pub(crate) fn launch_record(launch: ac_store::GameLaunch) -> ac_launcher::LaunchRecord {
    ac_launcher::LaunchRecord {
        uuid: launch.uuid,
        launched_at: launch.launched_at,
        pid: launch.pid,
        seen_at: launch.seen_at,
        before_pids: launch.before_pids,
    }
}

pub(crate) fn game_launch(record: ac_launcher::LaunchRecord) -> ac_store::GameLaunch {
    ac_store::GameLaunch {
        uuid: record.uuid,
        launched_at: record.launched_at,
        pid: record.pid,
        seen_at: record.seen_at,
        before_pids: record.before_pids,
    }
}

fn count(value: i64) -> u64 {
    value.max(0).unsigned_abs()
}

pub(crate) fn log_summary(summary: ac_store::LogSummary) -> ac_api::LogSummary {
    ac_api::LogSummary {
        uuid: summary.uuid,
        name: summary.name,
        lines: count(summary.lines),
        active: summary.active,
    }
}

pub(crate) fn log_session(session: ac_store::LogSessionInfo) -> ac_api::LogSession {
    ac_api::LogSession {
        id: session.id.to_string(),
        started: session.started,
        ended: session.ended,
        active: session.active,
        lines: count(session.lines),
    }
}

pub(crate) fn log_line(line: ac_store::LogLine) -> ac_api::LogLine {
    ac_api::LogLine {
        at: line.at,
        text: line.text,
    }
}

pub(crate) fn log_stats(stats: ac_store::LogStats) -> ac_api::LogStats {
    ac_api::LogStats {
        total_accounts: count(stats.total_accounts),
        total_sessions: count(stats.total_sessions),
        active_now: count(stats.active_now),
        total_duration: stats.total_duration,
        avg_duration: stats.avg_duration,
        longest_duration: stats.longest_duration,
        longest_name: stats.longest_name,
        longest_uuid: stats.longest_uuid,
        most_launched_name: stats.most_launched_name,
        most_launched_uuid: stats.most_launched_uuid,
        most_launched_count: count(stats.most_launched_count),
        total_lines: count(stats.total_lines),
    }
}

fn presence(presence: ac_player::Presence) -> ac_api::Presence {
    match presence {
        ac_player::Presence::Online => ac_api::Presence::Online,
        ac_player::Presence::Offline => ac_api::Presence::Offline,
        ac_player::Presence::Hidden => ac_api::Presence::Hidden,
        ac_player::Presence::Unknown => ac_api::Presence::Unknown,
    }
}

pub(crate) fn player(info: &ac_player::PlayerInfo) -> ac_api::PlayerInfo {
    ac_api::PlayerInfo {
        name: info.name.clone(),
        group: info.group.clone(),
        staff: info.staff.clone(),
        donate: info.donate.clone(),
        color: info.color.clone(),
        name_color: info.name_color.clone(),
        label: info.label.clone(),
        registered_at: info.registered_at.clone(),
        online: presence(info.online),
        realm: info.realm.clone(),
        last_seen: info.last_seen.clone(),
        subscription: info.subscription.clone(),
        sub_ending: info.sub_ending.clone(),
        likes: info.likes,
        views: info.views,
        score: info.score,
        donate_color: info.donate_color.clone(),
    }
}

pub(crate) fn texture_kind(kind: ac_api::TextureKind) -> ac_player::TextureKind {
    match kind {
        ac_api::TextureKind::Skin => ac_player::TextureKind::Skin,
        ac_api::TextureKind::Cape => ac_player::TextureKind::Cape,
    }
}

pub(crate) fn update_info(info: ac_update::UpdateInfo) -> ac_api::UpdateInfo {
    ac_api::UpdateInfo {
        current: info.current,
        latest: info.latest,
        available: info.available,
        notes: info.notes,
        name: info.name,
        published_at: info.published_at,
    }
}

pub(crate) fn update_progress(progress: ac_update::UpdateProgress) -> ac_api::UpdateProgress {
    ac_api::UpdateProgress {
        active: progress.active,
        done: progress.done,
        downloaded: progress.downloaded,
        total: progress.total,
        error: progress.error,
    }
}
