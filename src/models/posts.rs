use chrono::NaiveDateTime;
use diesel::{self, PgConnection, RunQueryDsl, QueryDsl, ExpressionMethods, BelongingToDsl};
use diesel::dsl::any;
use serde_json;

use BASE_URL;
use activity_pub::{PUBLIC_VISIBILTY, ap_url};
use activity_pub::actor::Actor;
use activity_pub::object::Object;
use models::blogs::Blog;
use models::users::User;
use models::post_authors::PostAuthor;
use schema::posts;

#[derive(Queryable, Identifiable)]
pub struct Post {
    pub id: i32,
    pub blog_id: i32,
    pub slug: String,
    pub title: String,
    pub content: String,
    pub published: bool,
    pub license: String,
    pub creation_date: NaiveDateTime    
}

#[derive(Insertable)]
#[table_name = "posts"]
pub struct NewPost {
    pub blog_id: i32,    
    pub slug: String,
    pub title: String,
    pub content: String,
    pub published: bool,
    pub license: String
}

impl Post {
    pub fn insert (conn: &PgConnection, new: NewPost) -> Post {
        diesel::insert_into(posts::table)
            .values(new)
            .get_result(conn)
            .expect("Error saving new post")
    }

    pub fn get(conn: &PgConnection, id: i32) -> Option<Post> {
        posts::table.filter(posts::id.eq(id))
            .limit(1)
            .load::<Post>(conn)
            .expect("Error loading post by id")
            .into_iter().nth(0)
    }

    pub fn find_by_slug(conn: &PgConnection, slug: String) -> Option<Post> {
        posts::table.filter(posts::slug.eq(slug))
            .limit(1)
            .load::<Post>(conn)
            .expect("Error loading post by slug")
            .into_iter().nth(0)
    }

    pub fn get_authors(&self, conn: &PgConnection) -> Vec<User> {
        use schema::users;
        use schema::post_authors;
        let author_list = PostAuthor::belonging_to(self).select(post_authors::author_id);
        users::table.filter(users::id.eq(any(author_list))).load::<User>(conn).unwrap()
    }

    pub fn get_blog(&self, conn: &PgConnection) -> Blog {
        use schema::blogs;
        blogs::table.filter(blogs::id.eq(self.blog_id))
            .limit(1)
            .load::<Blog>(conn)
            .expect("Couldn't load blog associted to post")
            .into_iter().nth(0).unwrap()
    }
}

impl Object for Post {
    fn compute_id(&self, conn: &PgConnection) -> String {
        ap_url(format!("{}/{}/{}", BASE_URL.as_str(), self.get_blog(conn).actor_id, self.slug))
    }

    fn serialize(&self, conn: &PgConnection) -> serde_json::Value {
        json!({
            "type": "Article",
            "attributedTo": self.get_authors(conn).into_iter().map(|a| a.compute_id(conn)).collect::<Vec<String>>(),
            "content": self.content,
            // TODO: "image": "image",
            // TODO: "preview": "preview",
            // TODO: "published": "published",
            // TODO: "replies": "replies",
            // TODO: "summary": "summary",
            "tag": [],
            // TODO: "updated": "updated",
            // TODO: "url": "url",
            "to": [ PUBLIC_VISIBILTY ]
        })
    }
}

/// ActivityPub Object to make it possible to view posts in Mastodon/Pleroma
/// and interact with them from there.
pub struct PostNote {
    pub post: Post
}

impl Object for PostNote {
fn compute_id(&self, conn: &PgConnection) -> String {
        ap_url(format!("{}/{}/{}/note", BASE_URL.as_str(), self.post.get_blog(conn).actor_id, self.post.slug))
    }

    fn serialize(&self, conn: &PgConnection) -> serde_json::Value {
        json!({
            "type": "Note",
            "attributedTo": self.post.get_authors(conn).into_iter().map(|a| a.compute_id(conn)).collect::<Vec<String>>(),
            "content": format!("<b>{}</b> in {}", self.post.title, self.post.get_blog(conn).title),
            // TODO: "image": "image",
            // TODO: "preview": "preview",
            // TODO: "published": "published",
            // TODO: "replies": "replies",
            // TODO: "summary": "summary",
            "tag": [],
            // TODO: "updated": "updated",
            // TODO: "url": "url",
            "to": [ PUBLIC_VISIBILTY ]
        })
    }
}
