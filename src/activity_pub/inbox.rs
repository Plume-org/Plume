use diesel::PgConnection;
use serde_json;

use activity_pub::activity;
use activity_pub::actor::Actor;
use activity_pub::sign::*;
use models::blogs::Blog;
use models::comments::*;
use models::follows::{Follow, NewFollow};
use models::posts::{Post, NewPost};
use models::users::User;

pub trait Inbox: Actor + Sized {
    fn received(&self, conn: &PgConnection, act: serde_json::Value);

    fn save(&self, conn: &PgConnection, act: serde_json::Value) {
        match act["type"].as_str().unwrap() {
            "Create" => {
                match act["object"]["type"].as_str().unwrap() {
                    "Article" => {
                        Post::insert(conn, NewPost {
                            blog_id: 0, // TODO
                            slug: String::from(""), // TODO
                            title: String::from(""), // TODO
                            content: act["object"]["content"].as_str().unwrap().to_string(),
                            published: true,
                            license: String::from("CC-0"),
                            ap_url: act["object"]["url"].as_str().unwrap().to_string()
                        });
                    },
                    "Note" => {
                        let previous_comment = Comment::get_by_ap_url(conn, act["object"]["inReplyTo"].as_str().unwrap().to_string());
                        Comment::insert(conn, NewComment {
                            content: act["object"]["content"].as_str().unwrap().to_string(),
                            spoiler_text: act["object"]["summary"].as_str().unwrap_or("").to_string(),
                            ap_url: Some(act["object"]["id"].as_str().unwrap().to_string()),
                            in_response_to_id: previous_comment.clone().map(|c| c.id),
                            post_id: previous_comment
                                .map(|c| c.post_id)
                                .unwrap_or_else(|| Post::get_by_ap_url(conn, act["object"]["inReplyTo"].as_str().unwrap().to_string()).unwrap().id),
                            author_id: User::from_url(conn, act["actor"].as_str().unwrap().to_string()).unwrap().id,
                            sensitive: act["object"]["sensitive"].as_bool().unwrap_or(false)
                        });
                    }
                    x => println!("Received a new {}, but didn't saved it", x)
                }
            },
            "Follow" => {
                let follow_act = activity::Follow::deserialize(act.clone());
                let from = User::from_url(conn, act["actor"].as_str().unwrap().to_string()).unwrap();
                match User::from_url(conn, act["object"].as_str().unwrap().to_string()) {
                    Some(u) => self.accept_follow(conn, &from, &u, &follow_act, from.id, u.id),
                    None => {
                        let blog = Blog::from_url(conn, follow_act.get_target_id()).unwrap();
                        self.accept_follow(conn, &from, &blog, &follow_act, from.id, blog.id)
                    }
                };
                
                // TODO: notification
            }
            x => println!("Received unknow activity type: {}", x)
        }
    }

    fn accept_follow<A: Actor, B: Actor + Signer, T: activity::Activity>(
        &self,
        conn: &PgConnection,
        from: &A,
        target: &B,
        follow: &T,
        from_id: i32,
        target_id: i32
    ) {
        Follow::insert(conn, NewFollow {
            follower_id: from_id,
            following_id: target_id
        });

        let accept = activity::Accept::new(target, follow, conn);
        from.send_to_inbox(conn, target, accept)
    }
}
