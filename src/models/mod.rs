use diesel::{PgConnection, RunQueryDsl, select};

macro_rules! find_by {
    ($table:ident, $fn:ident, $($col:ident as $type:ident),+) => {
        /// Try to find a $table with a given $col
        pub fn $fn(conn: &PgConnection, $($col: $type),+) -> Option<Self> {
            $table::table
                $(.filter($table::$col.eq($col)))+
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

macro_rules! insert {
    ($table:ident, $from:ident) => {
        pub fn insert(conn: &PgConnection, new: $from) -> Self {
            diesel::insert_into($table::table)
                .values(new)
                .get_result(conn)
                .expect("Error saving new $table")
        }
    };
}

sql_function!(nextval, nextval_t, (seq: ::diesel::sql_types::Text) -> ::diesel::sql_types::BigInt);
sql_function!(setval, setval_t, (seq: ::diesel::sql_types::Text, val: ::diesel::sql_types::BigInt) -> ::diesel::sql_types::BigInt);

fn get_next_id(conn: &PgConnection, seq: &str) -> i32 {
    // We cant' use currval because it may fail if nextval have never been called before
    let next = select(nextval(seq)).get_result::<i64>(conn).expect("Next ID fail");
    select(setval(seq, next - 1)).get_result::<i64>(conn).expect("Reset ID fail");
    next as i32
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
