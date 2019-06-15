CREATE TABLE password_reset_requests (
  id SERIAL PRIMARY KEY,
  email VARCHAR NOT NULL,
  token VARCHAR NOT NULL,
  expiration_date TIMESTAMP NOT NULL
);

CREATE INDEX password_reset_requests_token ON password_reset_requests (token);
CREATE UNIQUE INDEX password_reset_requests_email ON password_reset_requests (email);
