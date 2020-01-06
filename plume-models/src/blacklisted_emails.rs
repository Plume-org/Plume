use diesel::{
    self, delete, dsl::*, ExpressionMethods, JoinOnDsl, QueryDsl, RunQueryDsl,
    TextExpressionMethods,
};
use rocket::request::FromForm;
use schema::email_blacklist;
use {Connection, Error, Result};

#[derive(Clone, Queryable, Identifiable)]
#[table_name = "email_blacklist"]
pub struct BlacklistedEmail {
    pub id: i32,
    pub email_address: String,
    pub note: String,
}

#[derive(Insertable, FromForm)]
#[table_name = "email_blacklist"]
pub struct NewBlacklistedEmail {
    pub email_address: String,
    pub note: String,
}

impl BlacklistedEmail {
    insert!(email_blacklist, NewBlacklistedEmail);
    get!(email_blacklist);
    find_by!(email_blacklist, find_by_id, id as i32);
    pub fn delete_entries(conn: &Connection, ids: Vec<i32>) -> Result<bool> {
        use diesel::delete;
        for i in ids {
            let be: BlacklistedEmail = BlacklistedEmail::find_by_id(&conn, i)?;
            delete(&be).execute(conn);
        }
        Ok(true)
    }
    pub fn find_for_domain(conn: &Connection, domain: &String) -> Result<Vec<BlacklistedEmail>> {
        let effective = format!("%{}", domain);
        email_blacklist::table
            .filter(email_blacklist::email_address.like(effective))
            .load::<BlacklistedEmail>(conn)
            .map_err(Error::from)
    }
    pub fn matches_blacklist(conn: &Connection, email: &String) -> Result<bool> {
        use diesel::expression::*;
        use diesel::sql_types::Text;
        select(exists(
            email_blacklist::table.filter(
                (email)
                    .into_sql::<Text>()
                    .like(email_blacklist::email_address),
            ),
        ))
        .get_result(conn)
        .map_err(Error::from)
    }
    pub fn page(conn: &Connection, (min, max): (i32, i32)) -> Result<Vec<BlacklistedEmail>> {
        email_blacklist::table
            .offset(min.into())
            .limit((max - min).into())
            .load::<BlacklistedEmail>(conn)
            .map_err(Error::from)
    }
    pub fn count(conn: &Connection) -> Result<i64> {
        email_blacklist::table
            .count()
            .get_result(conn)
            .map_err(Error::from)
    }
}
