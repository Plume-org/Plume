use diesel;
use diesel::{PgConnection, RunQueryDsl, QueryDsl, ExpressionMethods};
use schema::posts;

#[derive(Queryable, Identifiable)]
pub struct Post {
    pub id: i32,
    pub blog_id: i32,
    pub slug: String,
    pub title: String,
    pub content: String,
    pub published: bool,
    pub license: String
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
}
