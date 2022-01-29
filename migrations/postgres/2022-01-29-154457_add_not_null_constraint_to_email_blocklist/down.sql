ALTER TABLE email_blocklist ALTER COLUMN notification_text DROP NOT NULL;
ALTER TABLE email_blocklist ALTER COLUMN notify_user DROP NOT NULL;
ALTER TABLE email_blocklist ALTER COLUMN note DROP NOT NULL;
ALTER TABLE email_blocklist ALTER COLUMN email_address DROP NOT NULL;
