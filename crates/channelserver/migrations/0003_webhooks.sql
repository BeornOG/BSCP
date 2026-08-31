-- Bind webhooks to a guild channel (the legacy channel_path rows keep working).
ALTER TABLE channel_webhooks ADD COLUMN channel_id TEXT;
ALTER TABLE channel_webhooks ADD COLUMN created_by TEXT;
CREATE INDEX idx_channel_webhooks_channel ON channel_webhooks (channel_id);

-- Mark messages posted by a webhook so clients can style them.
ALTER TABLE channel_messages ADD COLUMN via_webhook TEXT;
