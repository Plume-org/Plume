-- Your SQL goes here
create table email_blocklist(id integer primary key,
                             email_address text,
                             note text,
                             notify_user boolean default false,
                             notification_text text);
