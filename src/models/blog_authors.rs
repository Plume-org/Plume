use diesel;
use diesel::{QueryDsl, RunQueryDsl, ExpressionMethods, PgConnection};
use schema::blog_authors;

#[derive(Queryable, Identifiable)]
pub struct BlogAuthor {
    pub id: i32,
    pub blog_id: i32,
    pub author_id: i32,
    pub is_owner: bool,
}

#[derive(Insertable)]
#[table_name = "blog_authors"]
pub struct NewBlogAuthor {
    pub blog_id: i32,
    pub author_id: i32,
    pub is_owner: bool,
}

impl BlogAuthor {
    pub fn insert (conn: &PgConnection, new: NewBlogAuthor) -> BlogAuthor {
        diesel::insert_into(blog_authors::table)
            .values(new)
            .get_result(conn)
            .expect("Error saving new blog")
    }

    pub fn get(conn: &PgConnection, id: i32) -> Option<BlogAuthor> {
        blog_authors::table.filter(blog_authors::id.eq(id))
            .limit(1)
            .load::<BlogAuthor>(conn)
            .expect("Error loading blog by id")
            .into_iter().nth(0)
    }
}
