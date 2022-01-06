CREATE TABLE email_signups (
  id SERIAL PRIMARY KEY,
  email VARCHAR NOT NULL,
  token VARCHAR NOT NULL,
  expiration_date TIMESTAMP NOT NULL
);

CREATE INDEX email_signups_token ON email_signups (token);
CREATE UNIQUE INDEX email_signups_token_requests_email ON email_signups (email);
