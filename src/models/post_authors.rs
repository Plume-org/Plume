use diesel::{self, PgConnection, QueryDsl, RunQueryDsl, ExpressionMethods};

use schema::post_authors;

#[derive(Queryable, Identifiable)]
pub struct PostAuthor {
    pub id: i32,
    pub post_id: i32,
    pub author_id: i32
}

#[derive(Insertable)]
#[table_name = "post_authors"]
pub struct NewPostAuthor {
    pub post_id: i32,
    pub author_id: i32
}

impl PostAuthor {
    pub fn insert (conn: &PgConnection, new: NewPostAuthor) -> PostAuthor {
        diesel::insert_into(post_authors::table)
            .values(new)
            .get_result(conn)
            .expect("Error saving new blog author")
    }

    pub fn get(conn: &PgConnection, id: i32) -> Option<PostAuthor> {
        post_authors::table.filter(post_authors::id.eq(id))
            .limit(1)
            .load::<PostAuthor>(conn)
            .expect("Error loading blog author by id")
            .into_iter().nth(0)
    }
}
