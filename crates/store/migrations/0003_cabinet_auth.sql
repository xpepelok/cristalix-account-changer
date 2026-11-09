CREATE TABLE account_auth (
    uuid TEXT PRIMARY KEY NOT NULL REFERENCES accounts (uuid) ON DELETE CASCADE,
    password BLOB,
    cabinet_session BLOB,
    session_saved INTEGER NOT NULL DEFAULT 0
) WITHOUT ROWID;
