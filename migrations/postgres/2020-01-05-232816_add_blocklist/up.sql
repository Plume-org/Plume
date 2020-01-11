-- Your SQL goes here
create table email_blocklist(id SERIAL primary key,
                             email_address text ,
                             note text,
                             notify_user boolean default false,
                             notification_text text);
