-- Your SQL goes here
CREATE TABLE email_blocklist(id SERIAL PRIMARY KEY,
                             email_address TEXT UNIQUE,
                             note TEXT,
                             notify_user BOOLEAN DEFAULT FALSE,
                             notification_text TEXT);
