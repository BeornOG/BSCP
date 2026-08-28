-- Special message kinds (call_invite, call_end, ...) carry structured data in `metadata`.
ALTER TABLE messages ADD COLUMN kind TEXT NOT NULL DEFAULT 'text';
ALTER TABLE messages ADD COLUMN metadata TEXT;
