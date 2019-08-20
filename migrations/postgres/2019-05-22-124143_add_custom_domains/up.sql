--- Adding custom domain to Blog as an optional field
ALTER TABLE blogs ADD COLUMN custom_domain VARCHAR DEFAULT NULL UNIQUE;
