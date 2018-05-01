use diesel::{self, PgConnection, ExpressionMethods, QueryDsl, RunQueryDsl};

use schema::follows;

#[derive(Queryable, Identifiable)]
pub struct Follow {
    pub id: i32,
    pub follower_id: i32,
    pub following_id: i32
}

#[derive(Insertable)]
#[table_name = "follows"]
pub struct NewFollow {
    pub follower_id: i32,
    pub following_id: i32
}

impl Follow {
    pub fn insert(conn: &PgConnection, new: NewFollow) -> Follow {
        diesel::insert_into(follows::table)
            .values(new)
            .get_result(conn)
            .expect("Unable to insert new follow")
    }

    pub fn get(conn: &PgConnection, id: i32) -> Option<Follow> {
        follows::table.filter(follows::id.eq(id))
            .limit(1)
            .load::<Follow>(conn)
            .expect("Unable to load follow by id")
            .into_iter().nth(0)
    }
}
