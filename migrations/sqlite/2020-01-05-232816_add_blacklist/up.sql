-- Your SQL goes here
create table email_blacklist(id integer primary key, email_address text,note text,
                             notify boolean default false, notification_text text);
