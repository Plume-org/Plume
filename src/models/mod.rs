// TODO: support multiple columns (see Like::find_by_user_on_post)
macro_rules! find_by {
    ($table:ident, $fn:ident, $col:ident as $type:ident) => {
        /// Try to find a $table with a given $col
        pub fn $fn(conn: &PgConnection, val: $type) -> Option<Self> {
            $table::table.filter($table::$col.eq(val))
                .limit(1)
                .load::<Self>(conn)
                .expect("Error loading $table by $col")
                .into_iter().nth(0)
        }
    };
}

macro_rules! get {
    ($table:ident) => {
        pub fn get(conn: &PgConnection, id: i32) -> Option<Self> {
            $table::table.filter($table::id.eq(id))
                .limit(1)
                .load::<Self>(conn)
                .expect("Error loading $table by id")
                .into_iter().nth(0)
        }
    };
}

pub mod blog_authors;
pub mod blogs;
pub mod comments;
pub mod follows;
pub mod instance;
pub mod likes;
pub mod notifications;
pub mod post_authors;
pub mod posts;
pub mod reshares;
pub mod users;
