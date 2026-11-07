CREATE TABLE meta (
    key TEXT PRIMARY KEY NOT NULL,
    value TEXT NOT NULL
) WITHOUT ROWID;

CREATE TABLE accounts (
    uuid TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL DEFAULT '',
    token BLOB,
    label TEXT NOT NULL DEFAULT '',
    pinned INTEGER NOT NULL DEFAULT 0,
    client TEXT NOT NULL DEFAULT '',
    profile TEXT NOT NULL DEFAULT '',
    ram INTEGER NOT NULL DEFAULT 0,
    min_graphics INTEGER NOT NULL DEFAULT 0,
    fullscreen INTEGER NOT NULL DEFAULT 0,
    discord_rpc INTEGER NOT NULL DEFAULT 0,
    auto_enter INTEGER NOT NULL DEFAULT 0,
    debug_mode INTEGER NOT NULL DEFAULT 0,
    render_distance INTEGER NOT NULL DEFAULT 0,
    max_fps INTEGER NOT NULL DEFAULT 0,
    animations INTEGER NOT NULL DEFAULT 0,
    fast_render INTEGER NOT NULL DEFAULT 0,
    minimal INTEGER NOT NULL DEFAULT 0,
    expires INTEGER NOT NULL DEFAULT 0,
    first_seen INTEGER NOT NULL DEFAULT 0,
    last_seen INTEGER NOT NULL DEFAULT 0,
    last_launched INTEGER NOT NULL DEFAULT 0,
    last_online INTEGER NOT NULL DEFAULT 0
) WITHOUT ROWID;

CREATE TABLE forgotten (
    uuid TEXT PRIMARY KEY NOT NULL,
    token BLOB
) WITHOUT ROWID;

CREATE TABLE account_groups (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    profile TEXT NOT NULL DEFAULT '',
    pinned INTEGER NOT NULL DEFAULT 0,
    sort_order INTEGER NOT NULL DEFAULT 0,
    position INTEGER NOT NULL
) WITHOUT ROWID;

CREATE TABLE group_members (
    group_id TEXT NOT NULL REFERENCES account_groups (id) ON DELETE CASCADE,
    account_uuid TEXT NOT NULL REFERENCES accounts (uuid) ON DELETE CASCADE,
    position INTEGER NOT NULL,
    PRIMARY KEY (group_id, account_uuid)
) WITHOUT ROWID;

CREATE INDEX group_members_by_account ON group_members (account_uuid);

CREATE TABLE settings (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    launcher TEXT NOT NULL DEFAULT 'jar' CHECK (launcher IN ('normal', 'jar', 'new', 'custom')),
    custom_launcher TEXT NOT NULL DEFAULT '',
    auto_play INTEGER NOT NULL DEFAULT 1,
    stats INTEGER NOT NULL DEFAULT 1,
    aggressive INTEGER NOT NULL DEFAULT 0,
    window_title INTEGER NOT NULL DEFAULT 0,
    background TEXT,
    background_dim INTEGER NOT NULL DEFAULT 55 CHECK (background_dim BETWEEN 0 AND 100),
    background_blur INTEGER NOT NULL DEFAULT 0 CHECK (background_blur BETWEEN 0 AND 40)
);

INSERT INTO settings (id) VALUES (1);

CREATE TABLE player_cache (
    name TEXT PRIMARY KEY NOT NULL,
    info TEXT NOT NULL,
    fetched_at INTEGER NOT NULL
) WITHOUT ROWID;

CREATE TABLE game_launches (
    uuid TEXT PRIMARY KEY NOT NULL,
    position INTEGER NOT NULL,
    launched_at INTEGER NOT NULL,
    pid INTEGER NOT NULL DEFAULT 0,
    seen_at INTEGER,
    before_pids BLOB NOT NULL DEFAULT x''
) WITHOUT ROWID;

CREATE TABLE log_clients (
    uuid TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL DEFAULT ''
) WITHOUT ROWID;

CREATE TABLE log_sessions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    uuid TEXT NOT NULL REFERENCES log_clients (uuid) ON DELETE CASCADE,
    started INTEGER NOT NULL,
    ended INTEGER NOT NULL DEFAULT 0,
    active INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX log_sessions_by_client ON log_sessions (uuid, id);

CREATE TABLE log_lines (
    session_id INTEGER NOT NULL REFERENCES log_sessions (id) ON DELETE CASCADE,
    seq INTEGER NOT NULL,
    at INTEGER NOT NULL,
    text TEXT NOT NULL,
    PRIMARY KEY (session_id, seq)
) WITHOUT ROWID;
