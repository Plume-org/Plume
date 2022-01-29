CREATE TABLE email_blocklist2(id INTEGER PRIMARY KEY,
                              email_address TEXT UNIQUE,
                              note TEXT,
                              notify_user BOOLEAN DEFAULT FALSE,
                              notification_text TEXT);

INSERT INTO email_blocklist2 SELECT * FROM email_blocklist;
DROP TABLE email_blocklist;
ALTER TABLE email_blocklist2 RENAME TO email_blocklist;
