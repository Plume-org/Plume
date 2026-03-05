use activitystreams::activity::{Announce, Create, Delete, Follow, Like, Undo, Update};

use crate::{
    comments::Comment,
    follows, likes,
    posts::{Post, PostUpdate},
    reshares::Reshare,
    users::User,
    Connection, Error, CONFIG,
};
use plume_common::activity_pub::inbox::Inbox;

macro_rules! impl_into_inbox_result {
    ( $( $t:ty => $variant:ident ),+ ) => {
        $(
            impl From<$t> for InboxResult {
                fn from(x: $t) -> InboxResult {
                    InboxResult::$variant(x)
                }
            }
        )+
    }
}

pub enum InboxResult {
    Commented(Comment),
    Followed(follows::Follow),
    Liked(likes::Like),
    Other,
    Post(Post),
    Reshared(Reshare),
}

impl From<()> for InboxResult {
    fn from(_: ()) -> InboxResult {
        InboxResult::Other
    }
}

impl_into_inbox_result! {
    Comment => Commented,
    follows::Follow => Followed,
    likes::Like => Liked,
    Post => Post,
    Reshare => Reshared
}

#[async_recursion::async_recursion]
pub async fn inbox(conn: &mut Connection, act: serde_json::Value) -> Result<InboxResult, Error> {
    Inbox::handle(conn, act)
        .with::<User, Announce, Post>(CONFIG.proxy()).await
        .with::<User, Create, Comment>(CONFIG.proxy()).await
        .with::<User, Create, Post>(CONFIG.proxy()).await
        .with::<User, Delete, Comment>(CONFIG.proxy()).await
        .with::<User, Delete, Post>(CONFIG.proxy()).await
        .with::<User, Delete, User>(CONFIG.proxy()).await
        .with::<User, Follow, User>(CONFIG.proxy()).await
        .with::<User, Like, Post>(CONFIG.proxy()).await
        .with::<User, Undo, Reshare>(CONFIG.proxy()).await
        .with::<User, Undo, follows::Follow>(CONFIG.proxy()).await
        .with::<User, Undo, likes::Like>(CONFIG.proxy()).await
        .with::<User, Update, PostUpdate>(CONFIG.proxy()).await
        .done()
}

#[cfg(test)]
pub(crate) mod tests {
    use super::InboxResult;
    use crate::blogs::tests::fill_database as blog_fill_db;
    use crate::db_conn::DbConn;
    use crate::safe_string::SafeString;
    use crate::tests::db;
    use diesel::Connection;

    pub fn fill_database(
        conn: &DbConn,
    ) -> (
        Vec<crate::posts::Post>,
        Vec<crate::users::User>,
        Vec<crate::blogs::Blog>,
    ) {
        use crate::post_authors::*;
        use crate::posts::*;

        let (users, blogs) = blog_fill_db(conn);
        let post = Post::insert(
            conn,
            NewPost {
                blog_id: blogs[0].id,
                slug: "testing".to_owned(),
                title: "Testing".to_owned(),
                content: crate::safe_string::SafeString::new("Hello"),
                published: true,
                license: "WTFPL".to_owned(),
                creation_date: None,
                ap_url: format!("https://plu.me/~/{}/testing", blogs[0].actor_id),
                subtitle: "Bye".to_string(),
                source: "Hello".to_string(),
                cover_id: None,
            },
        )
        .unwrap();

        PostAuthor::insert(
            conn,
            NewPostAuthor {
                post_id: post.id,
                author_id: users[0].id,
            },
        )
        .unwrap();

        (vec![post], users, blogs)
    }

    #[test]
    fn announce_post() {
        let conn = db();
        conn.test_transaction::<_, (), _>(|| {
            let (posts, users, _) = fill_database(&conn);
            let act = json!({
                "id": "https://plu.me/announce/1",
                "actor": users[0].ap_url,
                "object": posts[0].ap_url,
                "type": "Announce",
            });

            match super::inbox(&conn, act).unwrap() {
                super::InboxResult::Reshared(r) => {
                    assert_eq!(r.post_id, posts[0].id);
                    assert_eq!(r.user_id, users[0].id);
                    assert_eq!(r.ap_url, "https://plu.me/announce/1".to_owned());
                }
                _ => panic!("Unexpected result"),
            };
            Ok(())
        });
    }

    #[test]
    fn create_comment() {
        let conn = db();
        conn.test_transaction::<_, (), _>(|| {
            let (posts, users, _) = fill_database(&conn);
            let act = json!({
                "id": "https://plu.me/comment/1/activity",
                "actor": users[0].ap_url,
                "object": {
                    "type": "Note",
                    "id": "https://plu.me/comment/1",
                    "attributedTo": users[0].ap_url,
                    "inReplyTo": posts[0].ap_url,
                    "content": "Hello.",
                    "to": [plume_common::activity_pub::PUBLIC_VISIBILITY]
                },
                "type": "Create",
            });

            match super::inbox(&conn, act).unwrap() {
                super::InboxResult::Commented(c) => {
                    assert_eq!(c.author_id, users[0].id);
                    assert_eq!(c.post_id, posts[0].id);
                    assert_eq!(c.in_response_to_id, None);
                    assert_eq!(c.content, SafeString::new("Hello."));
                    assert!(c.public_visibility);
                }
                _ => panic!("Unexpected result"),
            };
            Ok(())
        });
    }

    #[test]
    fn spoof_comment() {
        let conn = db();
        conn.test_transaction::<_, (), _>(|| {
            let (posts, users, _) = fill_database(&conn);
            let act = json!({
                "id": "https://plu.me/comment/1/activity",
                "actor": users[0].ap_url,
                "object": {
                    "type": "Note",
                    "id": "https://plu.me/comment/1",
                    "attributedTo": users[1].ap_url,
                    "inReplyTo": posts[0].ap_url,
                    "content": "Hello.",
                    "to": [plume_common::activity_pub::PUBLIC_VISIBILITY]
                },
                "type": "Create",
            });

            assert!(matches!(
                super::inbox(&conn, act),
                Err(super::Error::Inbox(
                    box plume_common::activity_pub::inbox::InboxError::InvalidObject(_),
                ))
            ));
            Ok(())
        });
    }

    #[test]
    fn spoof_comment_by_object_with_id() {
        let conn = db();
        conn.test_transaction::<_, (), _>(|| {
            let (posts, users, _) = fill_database(&conn);
            let act = json!({
                "id": "https://plu.me/comment/1/activity",
                "actor": users[0].ap_url,
                "object": {
                    "type": "Note",
                    "id": "https://plu.me/comment/1",
                    "attributedTo": {
                        "id": users[1].ap_url
                    },
                    "inReplyTo": posts[0].ap_url,
                    "content": "Hello.",
                    "to": [plume_common::activity_pub::PUBLIC_VISIBILITY]
                },
                "type": "Create",
            });

            assert!(matches!(
                super::inbox(&conn, act),
                Err(super::Error::Inbox(
                    box plume_common::activity_pub::inbox::InboxError::InvalidObject(_),
                ))
            ));
            Ok(())
        });
    }
    #[test]
    fn spoof_comment_by_object_without_id() {
        let conn = db();
        conn.test_transaction::<_, (), _>(|| {
            let (posts, users, _) = fill_database(&conn);
            let act = json!({
                "id": "https://plu.me/comment/1/activity",
                "actor": users[0].ap_url,
                "object": {
                    "type": "Note",
                    "id": "https://plu.me/comment/1",
                    "attributedTo": {},
                    "inReplyTo": posts[0].ap_url,
                    "content": "Hello.",
                    "to": [plume_common::activity_pub::PUBLIC_VISIBILITY]
                },
                "type": "Create",
            });

            assert!(matches!(
                super::inbox(&conn, act),
                Err(super::Error::Inbox(
                    box plume_common::activity_pub::inbox::InboxError::InvalidObject(_),
                ))
            ));
            Ok(())
        });
    }

    #[test]
    fn create_post() {
        let conn = db();
        conn.test_transaction::<_, (), _>(|| {
            let (_, users, blogs) = fill_database(&conn);
            let act = json!({
                "id": "https://plu.me/comment/1/activity",
                "actor": users[0].ap_url,
                "object": {
                    "type": "Article",
                    "id": "https://plu.me/~/BlogName/testing",
                    "attributedTo": [users[0].ap_url, blogs[0].ap_url],
                    "content": "Hello.",
                    "name": "My Article",
                    "summary": "Bye.",
                    "source": {
                        "content": "Hello.",
                        "mediaType": "text/markdown"
                    },
                    "published": "2014-12-12T12:12:12Z",
                    "to": [plume_common::activity_pub::PUBLIC_VISIBILITY]
                },
                "type": "Create",
            });

            match super::inbox(&conn, act).unwrap() {
                super::InboxResult::Post(p) => {
                    assert!(p.is_author(&conn, users[0].id).unwrap());
                    assert_eq!(p.source, "Hello".to_owned());
                    assert_eq!(p.blog_id, blogs[0].id);
                    assert_eq!(p.content, SafeString::new("Hello"));
                    assert_eq!(p.subtitle, "Bye".to_owned());
                    assert_eq!(p.title, "Testing".to_owned());
                }
                _ => panic!("Unexpected result"),
            };
            Ok(())
        });
    }

    #[test]
    fn spoof_post() {
        let conn = db();
        conn.test_transaction::<_, (), _>(|| {
            let (_, users, blogs) = fill_database(&conn);
            let act = json!({
                "id": "https://plu.me/comment/1/activity",
                "actor": users[0].ap_url,
                "object": {
                    "type": "Article",
                    "id": "https://plu.me/~/Blog/my-article",
                    "attributedTo": [users[1].ap_url, blogs[0].ap_url],
                    "content": "Hello.",
                    "name": "My Article",
                    "summary": "Bye.",
                    "source": {
                        "content": "Hello.",
                        "mediaType": "text/markdown"
                    },
                    "published": "2014-12-12T12:12:12Z",
                    "to": [plume_common::activity_pub::PUBLIC_VISIBILITY]
                },
                "type": "Create",
            });

            assert!(matches!(
                super::inbox(&conn, act),
                Err(super::Error::Inbox(
                    box plume_common::activity_pub::inbox::InboxError::InvalidObject(_),
                ))
            ));
            Ok(())
        });
    }

    #[test]
    fn spoof_post_by_object_with_id() {
        let conn = db();
        conn.test_transaction::<_, (), _>(|| {
            let (_, users, blogs) = fill_database(&conn);
            let act = json!({
                "id": "https://plu.me/comment/1/activity",
                "actor": users[0].ap_url,
                "object": {
                    "type": "Article",
                    "id": "https://plu.me/~/Blog/my-article",
                    "attributedTo": [
                        {"id": users[1].ap_url},
                        blogs[0].ap_url
                    ],
                    "content": "Hello.",
                    "name": "My Article",
                    "summary": "Bye.",
                    "source": {
                        "content": "Hello.",
                        "mediaType": "text/markdown"
                    },
                    "published": "2014-12-12T12:12:12Z",
                    "to": [plume_common::activity_pub::PUBLIC_VISIBILITY]
                },
                "type": "Create",
            });

            assert!(matches!(
                super::inbox(&conn, act),
                Err(super::Error::Inbox(
                    box plume_common::activity_pub::inbox::InboxError::InvalidObject(_),
                ))
            ));
            Ok(())
        });
    }

    #[test]
    fn spoof_post_by_object_without_id() {
        let conn = db();
        conn.test_transaction::<_, (), _>(|| {
            let (_, users, blogs) = fill_database(&conn);
            let act = json!({
                "id": "https://plu.me/comment/1/activity",
                "actor": users[0].ap_url,
                "object": {
                    "type": "Article",
                    "id": "https://plu.me/~/Blog/my-article",
                    "attributedTo": [{}, blogs[0].ap_url],
                    "content": "Hello.",
                    "name": "My Article",
                    "summary": "Bye.",
                    "source": {
                        "content": "Hello.",
                        "mediaType": "text/markdown"
                    },
                    "published": "2014-12-12T12:12:12Z",
                    "to": [plume_common::activity_pub::PUBLIC_VISIBILITY]
                },
                "type": "Create",
            });

            assert!(matches!(
                super::inbox(&conn, act),
                Err(super::Error::Inbox(
                    box plume_common::activity_pub::inbox::InboxError::InvalidObject(_),
                ))
            ));
            Ok(())
        });
    }

    #[test]
    fn delete_comment() {
        use crate::comments::*;

        let conn = db();
        conn.test_transaction::<_, (), _>(|| {
            let (posts, users, _) = fill_database(&conn);
            Comment::insert(
                &conn,
                NewComment {
                    content: SafeString::new("My comment"),
                    in_response_to_id: None,
                    post_id: posts[0].id,
                    author_id: users[0].id,
                    ap_url: Some("https://plu.me/comment/1".to_owned()),
                    sensitive: false,
                    spoiler_text: "spoiler".to_owned(),
                    public_visibility: true,
                },
            )
            .unwrap();

            let fail_act = json!({
                "id": "https://plu.me/comment/1/delete",
                "actor": users[1].ap_url, // Not the author of the comment, it should fail
                "object": "https://plu.me/comment/1",
                "type": "Delete",
            });
            assert!(super::inbox(&conn, fail_act).is_err());

            let ok_act = json!({
                "id": "https://plu.me/comment/1/delete",
                "actor": users[0].ap_url,
                "object": "https://plu.me/comment/1",
                "type": "Delete",
            });
            assert!(super::inbox(&conn, ok_act).is_ok());
            Ok(())
        })
    }

    #[test]
    fn delete_post() {
        let conn = db();
        conn.test_transaction::<_, (), _>(|| {
            let (posts, users, _) = fill_database(&conn);

            let fail_act = json!({
                "id": "https://plu.me/comment/1/delete",
                "actor": users[1].ap_url, // Not the author of the post, it should fail
                "object": posts[0].ap_url,
                "type": "Delete",
            });
            assert!(super::inbox(&conn, fail_act).is_err());

            let ok_act = json!({
                "id": "https://plu.me/comment/1/delete",
                "actor": users[0].ap_url,
                "object": posts[0].ap_url,
                "type": "Delete",
            });
            assert!(super::inbox(&conn, ok_act).is_ok());
            Ok(())
        });
    }

    #[test]
    fn delete_user() {
        let conn = db();
        conn.test_transaction::<_, (), _>(|| {
            let (_, users, _) = fill_database(&conn);

            let fail_act = json!({
                "id": "https://plu.me/@/Admin#delete",
                "actor": users[1].ap_url, // Not the same account
                "object": users[0].ap_url,
                "type": "Delete",
            });
            assert!(super::inbox(&conn, fail_act).is_err());

            let ok_act = json!({
                "id": "https://plu.me/@/Admin#delete",
                "actor": users[0].ap_url,
                "object": users[0].ap_url,
                "type": "Delete",
            });
            assert!(super::inbox(&conn, ok_act).is_ok());
            assert!(crate::users::User::get(&conn, users[0].id).is_err());

            Ok(())
        });
    }

    #[test]
    fn follow() {
        let conn = db();
        conn.test_transaction::<_, (), _>(|| {
            let (_, users, _) = fill_database(&conn);

            let act = json!({
                "id": "https://plu.me/follow/1",
                "actor": users[0].ap_url,
                "object": users[1].ap_url,
                "type": "Follow",
            });
            match super::inbox(&conn, act).unwrap() {
                InboxResult::Followed(f) => {
                    assert_eq!(f.follower_id, users[0].id);
                    assert_eq!(f.following_id, users[1].id);
                    assert_eq!(f.ap_url, "https://plu.me/follow/1".to_owned());
                }
                _ => panic!("Unexpected result"),
            }
            Ok(())
        });
    }

    #[test]
    fn like() {
        let conn = db();
        conn.test_transaction::<_, (), _>(|| {
            let (posts, users, _) = fill_database(&conn);

            let act = json!({
                "id": "https://plu.me/like/1",
                "actor": users[1].ap_url,
                "object": posts[0].ap_url,
                "type": "Like",
            });
            match super::inbox(&conn, act).unwrap() {
                InboxResult::Liked(l) => {
                    assert_eq!(l.user_id, users[1].id);
                    assert_eq!(l.post_id, posts[0].id);
                    assert_eq!(l.ap_url, "https://plu.me/like/1".to_owned());
                }
                _ => panic!("Unexpected result"),
            }
            Ok(())
        });
    }

    #[test]
    fn undo_reshare() {
        use crate::reshares::*;

        let conn = db();
        conn.test_transaction::<_, (), _>(|| {
            let (posts, users, _) = fill_database(&conn);

            let announce = Reshare::insert(
                &conn,
                NewReshare {
                    post_id: posts[0].id,
                    user_id: users[1].id,
                    ap_url: "https://plu.me/announce/1".to_owned(),
                },
            )
            .unwrap();

            let fail_act = json!({
                "id": "https://plu.me/undo/1",
                "actor": users[0].ap_url,
                "object": announce.ap_url,
                "type": "Undo",
            });
            assert!(super::inbox(&conn, fail_act).is_err());

            let ok_act = json!({
                "id": "https://plu.me/undo/1",
                "actor": users[1].ap_url,
                "object": announce.ap_url,
                "type": "Undo",
            });
            assert!(super::inbox(&conn, ok_act).is_ok());
            Ok(())
        });
    }

    #[test]
    fn undo_follow() {
        use crate::follows::*;

        let conn = db();
        conn.test_transaction::<_, (), _>(|| {
            let (_, users, _) = fill_database(&conn);

            let follow = Follow::insert(
                &conn,
                NewFollow {
                    follower_id: users[0].id,
                    following_id: users[1].id,
                    ap_url: "https://plu.me/follow/1".to_owned(),
                },
            )
            .unwrap();

            let fail_act = json!({
                "id": "https://plu.me/undo/1",
                "actor": users[2].ap_url,
                "object": follow.ap_url,
                "type": "Undo",
            });
            assert!(super::inbox(&conn, fail_act).is_err());

            let ok_act = json!({
                "id": "https://plu.me/undo/1",
                "actor": users[0].ap_url,
                "object": follow.ap_url,
                "type": "Undo",
            });
            assert!(super::inbox(&conn, ok_act).is_ok());
            Ok(())
        });
    }

    #[test]
    fn undo_like() {
        use crate::likes::*;

        let conn = db();
        conn.test_transaction::<_, (), _>(|| {
            let (posts, users, _) = fill_database(&conn);

            let like = Like::insert(
                &conn,
                NewLike {
                    post_id: posts[0].id,
                    user_id: users[1].id,
                    ap_url: "https://plu.me/like/1".to_owned(),
                },
            )
            .unwrap();

            let fail_act = json!({
                "id": "https://plu.me/undo/1",
                "actor": users[0].ap_url,
                "object": like.ap_url,
                "type": "Undo",
            });
            assert!(super::inbox(&conn, fail_act).is_err());

            let ok_act = json!({
                "id": "https://plu.me/undo/1",
                "actor": users[1].ap_url,
                "object": like.ap_url,
                "type": "Undo",
            });
            assert!(super::inbox(&conn, ok_act).is_ok());
            Ok(())
        });
    }

    #[test]
    fn update_post() {
        let conn = db();
        conn.test_transaction::<_, (), _>(|| {
            let (posts, users, _) = fill_database(&conn);

            let act = json!({
                "id": "https://plu.me/update/1",
                "actor": users[0].ap_url,
                "object": {
                    "type": "Article",
                    "id": posts[0].ap_url,
                    "name": "Mia Artikolo",
                    "summary": "Jes, mi parolas esperanton nun",
                    "content": "<b>Saluton</b>, mi skribas testojn",
                    "source": {
                        "mediaType": "text/markdown",
                        "content": "**Saluton**, mi skribas testojn"
                    },
                },
                "type": "Update",
            });

            super::inbox(&conn, act).unwrap();
            Ok(())
        });
    }
}
