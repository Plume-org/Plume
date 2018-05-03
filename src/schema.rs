table! {
    blog_authors (id) {
        id -> Int4,
        blog_id -> Int4,
        author_id -> Int4,
        is_owner -> Bool,
    }
}

table! {
    blogs (id) {
        id -> Int4,
        actor_id -> Varchar,
        title -> Varchar,
        summary -> Text,
        outbox_url -> Varchar,
        inbox_url -> Varchar,
        instance_id -> Int4,
        creation_date -> Timestamp,
        ap_url -> Text,
    }
}

table! {
    follows (id) {
        id -> Int4,
        follower_id -> Int4,
        following_id -> Int4,
    }
}

table! {
    instances (id) {
        id -> Int4,
        public_domain -> Varchar,
        name -> Varchar,
        local -> Bool,
        blocked -> Bool,
        creation_date -> Timestamp,
    }
}

table! {
    post_authors (id) {
        id -> Int4,
        post_id -> Int4,
        author_id -> Int4,
    }
}

table! {
    posts (id) {
        id -> Int4,
        blog_id -> Int4,
        slug -> Varchar,
        title -> Varchar,
        content -> Text,
        published -> Bool,
        license -> Varchar,
        creation_date -> Timestamp,
    }
}

table! {
    users (id) {
        id -> Int4,
        username -> Varchar,
        display_name -> Varchar,
        outbox_url -> Varchar,
        inbox_url -> Varchar,
        is_admin -> Bool,
        summary -> Text,
        email -> Nullable<Text>,
        hashed_password -> Nullable<Text>,
        instance_id -> Int4,
        creation_date -> Timestamp,
        ap_url -> Text,
        private_key -> Nullable<Text>,
        public_key -> Text,
    }
}

joinable!(blog_authors -> blogs (blog_id));
joinable!(blog_authors -> users (author_id));
joinable!(blogs -> instances (instance_id));
joinable!(post_authors -> posts (post_id));
joinable!(post_authors -> users (author_id));
joinable!(posts -> blogs (blog_id));
joinable!(users -> instances (instance_id));

allow_tables_to_appear_in_same_query!(
    blog_authors,
    blogs,
    follows,
    instances,
    post_authors,
    posts,
    users,
);
