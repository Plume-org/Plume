use chrono::NaiveDateTime;
use diesel::{self, PgConnection, QueryDsl, RunQueryDsl, ExpressionMethods};

use schema::reshares;

#[derive(Serialize, Deserialize, Queryable, Identifiable)]
pub struct Reshare {
    id: i32,
    user_id: i32,
    post_id: i32,
    ap_url: String,
    creation_date: NaiveDateTime
}

#[derive(Insertable)]
#[table_name = "reshares"]
pub struct NewReshare {
    user_id: i32,
    post_id: i32,
    ap_url: String
}

impl Reshare {
    pub fn insert(conn: &PgConnection, new: NewReshare) -> Reshare {
        diesel::insert_into(reshares::table)
            .values(new)
            .get_result(conn)
            .expect("Couldn't save reshare")
    }

    pub fn get(conn: &PgConnection, id: i32) -> Option<Reshare> {
        reshares::table.filter(reshares::id.eq(id))
            .limit(1)
            .load::<Reshare>(conn)
            .expect("Could'nt load reshare")
            .into_iter().nth(0)
    }
}
