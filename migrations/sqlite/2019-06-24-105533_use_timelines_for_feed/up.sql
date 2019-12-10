-- Your SQL goes here
INSERT INTO timeline_definition (name, query) VALUES
       ('Local feed', 'local'),
       ('Federated feed', 'all');
INSERT INTO timeline_definition (user_id,name,query)
               select id,'Your feed','followed or ['||fqn||']' from users;
