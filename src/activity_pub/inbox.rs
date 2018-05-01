use diesel::PgConnection;
use diesel::associations::Identifiable;
use serde_json;

use activity_pub::activity::Activity;
use activity_pub::actor::Actor;
use models::blogs::Blog;
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
                            blog_id: 0,    
                            slug: String::from(""),
                            title: String::from(""),
                            content: act["object"]["content"].as_str().unwrap().to_string(),
                            published: true,
                            license: String::from("CC-0")
                        });
                    },
                    x => println!("Received a new {}, but didn't saved it", x)
                }
            },
            "Follow" => {
                let follow_id = act["object"].as_str().unwrap().to_string();
                let from = User::from_url(conn, act["actor"].as_str().unwrap().to_string()).unwrap();
                match User::from_url(conn, act["object"].as_str().unwrap().to_string()) {
                    Some(u) => self.accept_follow(conn, &from, &u, follow_id, from.id, u.id),
                    None => {
                        let blog = Blog::from_url(conn, follow_id.clone()).unwrap();
                        self.accept_follow(conn, &from, &blog, follow_id, from.id, blog.id)
                    }
                };
                
                // TODO: notification
            }
            x => println!("Received unknow activity type: {}", x)
        }
    }

    fn accept_follow<A: Actor, B: Actor>(&self, conn: &PgConnection, from: &A, target: &B, follow_id: String, from_id: i32, target_id: i32) {
        Follow::insert(conn, NewFollow {
            follower_id: from_id,
            following_id: target_id
        });

        let accept = Activity::accept(target, follow_id, conn);
        from.send_to_inbox(conn, accept)
    }
}
