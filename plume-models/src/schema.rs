table! {
    api_tokens (id) {
        id -> Int4,
        creation_date -> Timestamp,
        value -> Text,
        scopes -> Text,
        app_id -> Int4,
        user_id -> Int4,
    }
}

table! {
    apps (id) {
        id -> Int4,
        name -> Text,
        client_id -> Text,
        client_secret -> Text,
        redirect_uri -> Nullable<Text>,
        website -> Nullable<Text>,
        creation_date -> Timestamp,
    }
}

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
        private_key -> Nullable<Text>,
        public_key -> Text,
        fqn -> Text,
        summary_html -> Text,
        icon_id -> Nullable<Int4>,
        banner_id -> Nullable<Int4>,
        theme -> Nullable<Varchar>,
    }
}

table! {
    comments (id) {
        id -> Int4,
        content -> Text,
        in_response_to_id -> Nullable<Int4>,
        post_id -> Int4,
        author_id -> Int4,
        creation_date -> Timestamp,
        ap_url -> Nullable<Varchar>,
        sensitive -> Bool,
        spoiler_text -> Text,
        public_visibility -> Bool,
    }
}

table! {
    comment_seers (id) {
        id -> Int4,
        comment_id -> Int4,
        user_id -> Int4,
    }
}

table! {
    email_blocklist (id) {
        id -> Int4,
        email_address -> Text,
        note -> Text,
        notify_user -> Bool,
        notification_text -> Text,
    }
}

table! {
    email_signups (id) {
        id -> Int4,
        email -> Varchar,
        token -> Varchar,
        expiration_date -> Timestamp,
    }
}

table! {
    follows (id) {
        id -> Int4,
        follower_id -> Int4,
        following_id -> Int4,
        ap_url -> Text,
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
        open_registrations -> Bool,
        short_description -> Text,
        long_description -> Text,
        default_license -> Text,
        long_description_html -> Varchar,
        short_description_html -> Varchar,
    }
}

table! {
    likes (id) {
        id -> Int4,
        user_id -> Int4,
        post_id -> Int4,
        creation_date -> Timestamp,
        ap_url -> Varchar,
    }
}

table! {
    list_elems (id) {
        id -> Int4,
        list_id -> Int4,
        user_id -> Nullable<Int4>,
        blog_id -> Nullable<Int4>,
        word -> Nullable<Varchar>,
    }
}

table! {
    lists (id) {
        id -> Int4,
        name -> Varchar,
        user_id -> Nullable<Int4>,
        #[sql_name = "type"]
        type_ -> Int4,
    }
}

table! {
    medias (id) {
        id -> Int4,
        file_path -> Text,
        alt_text -> Text,
        is_remote -> Bool,
        remote_url -> Nullable<Text>,
        sensitive -> Bool,
        content_warning -> Nullable<Text>,
        owner_id -> Int4,
    }
}

table! {
    mentions (id) {
        id -> Int4,
        mentioned_id -> Int4,
        post_id -> Nullable<Int4>,
        comment_id -> Nullable<Int4>,
    }
}

table! {
    notifications (id) {
        id -> Int4,
        user_id -> Int4,
        creation_date -> Timestamp,
        kind -> Varchar,
        object_id -> Int4,
    }
}

table! {
    password_reset_requests (id) {
        id -> Int4,
        email -> Varchar,
        token -> Varchar,
        expiration_date -> Timestamp,
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
        ap_url -> Varchar,
        subtitle -> Text,
        source -> Text,
        cover_id -> Nullable<Int4>,
    }
}

table! {
    reshares (id) {
        id -> Int4,
        user_id -> Int4,
        post_id -> Int4,
        ap_url -> Varchar,
        creation_date -> Timestamp,
    }
}

table! {
    tags (id) {
        id -> Int4,
        tag -> Text,
        is_hashtag -> Bool,
        post_id -> Int4,
    }
}

table! {
    timeline (id) {
        id -> Int4,
        post_id -> Int4,
        timeline_id -> Int4,
    }
}

table! {
    timeline_definition (id) {
        id -> Int4,
        user_id -> Nullable<Int4>,
        name -> Varchar,
        query -> Varchar,
    }
}

table! {
    users (id) {
        id -> Int4,
        username -> Varchar,
        display_name -> Varchar,
        outbox_url -> Varchar,
        inbox_url -> Varchar,
        summary -> Text,
        email -> Nullable<Text>,
        hashed_password -> Nullable<Text>,
        instance_id -> Int4,
        creation_date -> Timestamp,
        ap_url -> Text,
        private_key -> Nullable<Text>,
        public_key -> Text,
        shared_inbox_url -> Nullable<Varchar>,
        followers_endpoint -> Varchar,
        avatar_id -> Nullable<Int4>,
        last_fetched_date -> Timestamp,
        fqn -> Text,
        summary_html -> Text,
        role -> Int4,
        preferred_theme -> Nullable<Varchar>,
        hide_custom_css -> Bool,
    }
}

joinable!(api_tokens -> apps (app_id));
joinable!(api_tokens -> users (user_id));
joinable!(blog_authors -> blogs (blog_id));
joinable!(blog_authors -> users (author_id));
joinable!(blogs -> instances (instance_id));
joinable!(comment_seers -> comments (comment_id));
joinable!(comment_seers -> users (user_id));
joinable!(comments -> posts (post_id));
joinable!(comments -> users (author_id));
joinable!(likes -> posts (post_id));
joinable!(likes -> users (user_id));
joinable!(list_elems -> blogs (blog_id));
joinable!(list_elems -> lists (list_id));
joinable!(list_elems -> users (user_id));
joinable!(lists -> users (user_id));
joinable!(mentions -> comments (comment_id));
joinable!(mentions -> posts (post_id));
joinable!(mentions -> users (mentioned_id));
joinable!(notifications -> users (user_id));
joinable!(post_authors -> posts (post_id));
joinable!(post_authors -> users (author_id));
joinable!(posts -> blogs (blog_id));
joinable!(posts -> medias (cover_id));
joinable!(reshares -> posts (post_id));
joinable!(reshares -> users (user_id));
joinable!(tags -> posts (post_id));
joinable!(timeline -> posts (post_id));
joinable!(timeline -> timeline_definition (timeline_id));
joinable!(timeline_definition -> users (user_id));
joinable!(users -> instances (instance_id));

allow_tables_to_appear_in_same_query!(
    api_tokens,
    apps,
    blog_authors,
    blogs,
    comments,
    comment_seers,
    email_blocklist,
    email_signups,
    follows,
    instances,
    likes,
    list_elems,
    lists,
    medias,
    mentions,
    notifications,
    password_reset_requests,
    post_authors,
    posts,
    reshares,
    tags,
    timeline,
    timeline_definition,
    users,
);
