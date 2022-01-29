ALTER TABLE email_blocklist ALTER COLUMN email_address SET NOT NULL;
ALTER TABLE email_blocklist ALTER COLUMN note SET NOT NULL;
ALTER TABLE email_blocklist ALTER COLUMN notify_user SET NOT NULL;
ALTER TABLE email_blocklist ALTER COLUMN notification_text SET NOT NULL;
