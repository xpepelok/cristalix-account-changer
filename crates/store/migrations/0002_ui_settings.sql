ALTER TABLE settings ADD COLUMN theme TEXT NOT NULL DEFAULT 'dark' CHECK (theme IN ('dark', 'light'));

ALTER TABLE settings ADD COLUMN sound INTEGER NOT NULL DEFAULT 1;
