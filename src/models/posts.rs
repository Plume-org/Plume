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
        ap_url(format!("{}/~/{}/{}", BASE_URL.as_str(), self.get_blog(conn).actor_id, self.slug))
    }

    fn serialize(&self, conn: &PgConnection) -> serde_json::Value {
        let followers = self.get_authors(conn).into_iter().map(|a| a.get_followers(conn)).collect::<Vec<Vec<User>>>();
        let mut to = followers.into_iter().fold(vec![], |mut acc, f| {
            for x in f {
                acc.push(x.ap_url);
            }
            acc
        });
        to.push(PUBLIC_VISIBILTY.to_string());

        json!({
            "type": "Article",
            "id": self.compute_id(conn),
            "attributedTo": self.get_authors(conn).into_iter().map(|a| a.compute_id(conn)).collect::<Vec<String>>(),
            "content": self.content,
            "actor": self.get_authors(conn)[0].compute_id(conn),
            "published": self.creation_date,
            // TODO: "image": "image",
            // TODO: "preview": "preview",
            // TODO: "published": "published",
            // TODO: "replies": "replies",
            // TODO: "summary": "summary",
            "tag": [],
            // TODO: "updated": "updated",
            // TODO: "url": "url",
            "to": to,
            "cc": []
        })
    }
}
