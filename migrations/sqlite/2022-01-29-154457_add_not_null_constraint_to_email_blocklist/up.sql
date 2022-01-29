CREATE TABLE email_blocklist2(id INTEGER PRIMARY KEY,
                              email_address TEXT UNIQUE NOT NULL,
                              note TEXT NOT NULL,
                              notify_user BOOLEAN DEFAULT FALSE NOT NULL,
                              notification_text TEXT NOT NULL);

INSERT INTO email_blocklist2 SELECT * FROM email_blocklist;
DROP TABLE email_blocklist;
ALTER TABLE email_blocklist2 RENAME TO email_blocklist;
