use diesel::PgConnection;
use serde_json;

use models::posts::{Post, NewPost};

pub trait Inbox {
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
            x => println!("Received unknow activity type: {}", x)
        }
    }
}
