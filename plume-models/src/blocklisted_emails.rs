use diesel::{self, ExpressionMethods, QueryDsl, RunQueryDsl, TextExpressionMethods};
use glob::Pattern;

use schema::email_blocklist;
use {Connection, Error, Result};

#[derive(Clone, Queryable, Identifiable)]
#[table_name = "email_blocklist"]
pub struct BlocklistedEmail {
    pub id: i32,
    pub email_address: String,
    pub note: String,
    pub notify_user: bool,
    pub notification_text: String,
}

#[derive(Insertable, FromForm)]
#[table_name = "email_blocklist"]
pub struct NewBlocklistedEmail {
    pub email_address: String,
    pub note: String,
    pub notify_user: bool,
    pub notification_text: String,
}

impl BlocklistedEmail {
    insert!(email_blocklist, NewBlocklistedEmail);
    get!(email_blocklist);
    find_by!(email_blocklist, find_by_id, id as i32);
    pub fn delete_entries(conn: &Connection, ids: Vec<i32>) -> Result<bool> {
        use diesel::delete;
        for i in ids {
            let be: BlocklistedEmail = BlocklistedEmail::find_by_id(&conn, i)?;
            delete(&be).execute(conn)?;
        }
        Ok(true)
    }
    pub fn find_for_domain(conn: &Connection, domain: &str) -> Result<Vec<BlocklistedEmail>> {
        let effective = format!("%{}", domain);
        email_blocklist::table
            .filter(email_blocklist::email_address.like(effective))
            .load::<BlocklistedEmail>(conn)
            .map_err(Error::from)
    }
    pub fn matches_blocklist(conn: &Connection, email: &str) -> Result<Option<BlocklistedEmail>> {
        let mut result = email_blocklist::table.load::<BlocklistedEmail>(conn)?;
        for i in result.drain(..) {
            if let Ok(x) = Pattern::new(&i.email_address) {
                if x.matches(email) {
                    return Ok(Some(i));
                }
            }
        }
        Ok(None)
    }
    pub fn page(conn: &Connection, (min, max): (i32, i32)) -> Result<Vec<BlocklistedEmail>> {
        email_blocklist::table
            .offset(min.into())
            .limit((max - min).into())
            .load::<BlocklistedEmail>(conn)
            .map_err(Error::from)
    }
    pub fn count(conn: &Connection) -> Result<i64> {
        email_blocklist::table
            .count()
            .get_result(conn)
            .map_err(Error::from)
    }
}
