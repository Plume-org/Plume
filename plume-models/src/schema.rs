table! {
    blog_authors (id) {
        id -> Nullable<Integer>,
        blog_id -> Integer,
        author_id -> Integer,
        is_owner -> Bool,
    }
}

table! {
    blogs (id) {
        id -> Nullable<Integer>,
        actor_id -> Text,
        title -> Text,
        summary -> Text,
        outbox_url -> Text,
        inbox_url -> Text,
        instance_id -> Integer,
        creation_date -> Integer,
        ap_url -> Text,
        private_key -> Nullable<Text>,
        public_key -> Text,
    }
}

table! {
    comments (id) {
        id -> Nullable<Integer>,
        content -> Text,
        in_response_to_id -> Nullable<Integer>,
        post_id -> Integer,
        author_id -> Integer,
        creation_date -> Integer,
        ap_url -> Nullable<Text>,
        sensitive -> Bool,
        spoiler_text -> Text,
    }
}

table! {
    follows (id) {
        id -> Nullable<Integer>,
        follower_id -> Integer,
        following_id -> Integer,
        ap_url -> Text,
    }
}

table! {
    instances (id) {
        id -> Nullable<Integer>,
        public_domain -> Text,
        name -> Text,
        local -> Bool,
        blocked -> Bool,
        creation_date -> Integer,
        open_registrations -> Bool,
        short_description -> Text,
        long_description -> Text,
        default_license -> Text,
        long_description_html -> Text,
        short_description_html -> Text,
    }
}

table! {
    likes (id) {
        id -> Nullable<Integer>,
        user_id -> Integer,
        post_id -> Integer,
        ap_url -> Text,
        creation_date -> Integer,
    }
}

table! {
    medias (id) {
        id -> Nullable<Integer>,
        file_path -> Text,
        alt_text -> Text,
        is_remote -> Bool,
        remote_url -> Nullable<Text>,
        sensitive -> Bool,
        content_warning -> Nullable<Text>,
        owner_id -> Integer,
    }
}

table! {
    mentions (id) {
        id -> Nullable<Integer>,
        mentioned_id -> Integer,
        post_id -> Nullable<Integer>,
        comment_id -> Nullable<Integer>,
        ap_url -> Text,
    }
}

table! {
    notifications (id) {
        id -> Nullable<Integer>,
        user_id -> Integer,
        creation_date -> Integer,
        kind -> Text,
        object_id -> Integer,
    }
}

table! {
    post_authors (id) {
        id -> Nullable<Integer>,
        post_id -> Integer,
        author_id -> Integer,
    }
}

table! {
    posts (id) {
        id -> Nullable<Integer>,
        blog_id -> Integer,
        slug -> Text,
        title -> Text,
        content -> Text,
        published -> Bool,
        license -> Text,
        creation_date -> Integer,
        ap_url -> Text,
        subtitle -> Text,
        source -> Text,
    }
}

table! {
    reshares (id) {
        id -> Nullable<Integer>,
        user_id -> Integer,
        post_id -> Integer,
        ap_url -> Text,
        creation_date -> Integer,
    }
}

table! {
    tags (id) {
        id -> Nullable<Integer>,
        tag -> Text,
        is_hastag -> Bool,
        post_id -> Integer,
    }
}

table! {
    users (id) {
        id -> Nullable<Integer>,
        username -> Text,
        display_name -> Text,
        outbox_url -> Text,
        inbox_url -> Text,
        is_admin -> Bool,
        summary -> Text,
        email -> Nullable<Text>,
        hashed_password -> Nullable<Text>,
        instance_id -> Integer,
        creation_date -> Integer,
        ap_url -> Text,
        private_key -> Nullable<Text>,
        public_key -> Text,
        shared_inbox_url -> Nullable<Text>,
        followers_endpoint -> Text,
        avatar_id -> Nullable<Integer>,
        last_fetched_date -> Timestamp,
    }
}

joinable!(blog_authors -> blogs (blog_id));
joinable!(blog_authors -> users (author_id));
joinable!(blogs -> instances (instance_id));
joinable!(comments -> posts (post_id));
joinable!(comments -> users (author_id));
joinable!(likes -> posts (post_id));
joinable!(likes -> users (user_id));
joinable!(mentions -> comments (comment_id));
joinable!(mentions -> posts (post_id));
joinable!(mentions -> users (mentioned_id));
joinable!(notifications -> users (user_id));
joinable!(post_authors -> posts (post_id));
joinable!(post_authors -> users (author_id));
joinable!(posts -> blogs (blog_id));
joinable!(reshares -> posts (post_id));
joinable!(reshares -> users (user_id));
joinable!(tags -> posts (post_id));
joinable!(users -> instances (instance_id));

allow_tables_to_appear_in_same_query!(
    blog_authors,
    blogs,
    comments,
    follows,
    instances,
    likes,
    medias,
    mentions,
    notifications,
    post_authors,
    posts,
    reshares,
    tags,
    users,
);
