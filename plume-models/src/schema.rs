table! {
    use diesel::sql_types::*;
    use crate::users::User_role;

    api_tokens (id) {
        id -> Integer,
        creation_date -> Timestamp,
        value -> Text,
        scopes -> Text,
        app_id -> Integer,
        user_id -> Integer,
    }
}

table! {
    use diesel::sql_types::*;
    use crate::users::User_role;

    apps (id) {
        id -> Integer,
        name -> Text,
        client_id -> Text,
        client_secret -> Text,
        redirect_uri -> Nullable<Text>,
        website -> Nullable<Text>,
        creation_date -> Timestamp,
    }
}

table! {
    use diesel::sql_types::*;
    use crate::users::User_role;

    blog_authors (id) {
        id -> Integer,
        blog_id -> Integer,
        author_id -> Integer,
        is_owner -> Bool,
    }
}

table! {
    use diesel::sql_types::*;
    use crate::users::User_role;

    blogs (id) {
        id -> Integer,
        actor_id -> Text,
        title -> Text,
        summary -> Text,
        outbox_url -> Text,
        inbox_url -> Text,
        instance_id -> Integer,
        creation_date -> Timestamp,
        ap_url -> Text,
        private_key -> Nullable<Text>,
        public_key -> Text,
        fqn -> Text,
        summary_html -> Text,
        icon_id -> Nullable<Integer>,
        banner_id -> Nullable<Integer>,
    }
}

table! {
    use diesel::sql_types::*;
    use crate::users::User_role;

    comment_seers (id) {
        id -> Integer,
        comment_id -> Integer,
        user_id -> Integer,
    }
}

table! {
    use diesel::sql_types::*;
    use crate::users::User_role;

    comments (id) {
        id -> Integer,
        content -> Text,
        in_response_to_id -> Nullable<Integer>,
        post_id -> Integer,
        author_id -> Integer,
        creation_date -> Timestamp,
        ap_url -> Nullable<Text>,
        sensitive -> Bool,
        spoiler_text -> Text,
        public_visibility -> Bool,
    }
}

table! {
    use diesel::sql_types::*;
    use crate::users::User_role;

    follows (id) {
        id -> Integer,
        follower_id -> Integer,
        following_id -> Integer,
        ap_url -> Text,
    }
}

table! {
    use diesel::sql_types::*;
    use crate::users::User_role;

    instances (id) {
        id -> Integer,
        public_domain -> Text,
        name -> Text,
        local -> Bool,
        blocked -> Bool,
        creation_date -> Timestamp,
        open_registrations -> Bool,
        short_description -> Text,
        long_description -> Text,
        default_license -> Text,
        long_description_html -> Text,
        short_description_html -> Text,
    }
}

table! {
    use diesel::sql_types::*;
    use crate::users::User_role;

    likes (id) {
        id -> Integer,
        user_id -> Integer,
        post_id -> Integer,
        creation_date -> Timestamp,
        ap_url -> Text,
    }
}

table! {
    use diesel::sql_types::*;
    use crate::users::User_role;

    medias (id) {
        id -> Integer,
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
    use diesel::sql_types::*;
    use crate::users::User_role;

    mentions (id) {
        id -> Integer,
        mentioned_id -> Integer,
        post_id -> Nullable<Integer>,
        comment_id -> Nullable<Integer>,
    }
}

table! {
    use diesel::sql_types::*;
    use crate::users::User_role;

    notifications (id) {
        id -> Integer,
        user_id -> Integer,
        creation_date -> Timestamp,
        kind -> Text,
        object_id -> Integer,
    }
}

table! {
    use diesel::sql_types::*;
    use crate::users::User_role;

    password_reset_requests (id) {
        id -> Integer,
        email -> Text,
        token -> Text,
        expiration_date -> Timestamp,
    }
}

table! {
    use diesel::sql_types::*;
    use crate::users::User_role;

    post_authors (id) {
        id -> Integer,
        post_id -> Integer,
        author_id -> Integer,
    }
}

table! {
    use diesel::sql_types::*;
    use crate::users::User_role;

    posts (id) {
        id -> Integer,
        blog_id -> Integer,
        slug -> Text,
        title -> Text,
        content -> Text,
        published -> Bool,
        license -> Text,
        creation_date -> Timestamp,
        ap_url -> Text,
        subtitle -> Text,
        source -> Text,
        cover_id -> Nullable<Integer>,
    }
}

table! {
    use diesel::sql_types::*;
    use crate::users::User_role;

    reshares (id) {
        id -> Integer,
        user_id -> Integer,
        post_id -> Integer,
        ap_url -> Text,
        creation_date -> Timestamp,
    }
}

table! {
    use diesel::sql_types::*;
    use crate::users::User_role;

    tags (id) {
        id -> Integer,
        tag -> Text,
        is_hashtag -> Bool,
        post_id -> Integer,
    }
}

table! {
    use diesel::sql_types::*;
    use crate::users::User_role;

    users (id) {
        id -> Integer,
        username -> Text,
        display_name -> Text,
        outbox_url -> Text,
        inbox_url -> Text,
        summary -> Text,
        email -> Nullable<Text>,
        hashed_password -> Nullable<Text>,
        instance_id -> Integer,
        creation_date -> Timestamp,
        ap_url -> Text,
        private_key -> Nullable<Text>,
        public_key -> Text,
        shared_inbox_url -> Nullable<Text>,
        followers_endpoint -> Text,
        avatar_id -> Nullable<Integer>,
        last_fetched_date -> Timestamp,
        fqn -> Text,
        summary_html -> Text,
        role -> Text,
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
joinable!(users -> instances (instance_id));

allow_tables_to_appear_in_same_query!(
    api_tokens,
    apps,
    blog_authors,
    blogs,
    comment_seers,
    comments,
    follows,
    instances,
    likes,
    medias,
    mentions,
    notifications,
    password_reset_requests,
    post_authors,
    posts,
    reshares,
    tags,
    users,
);
