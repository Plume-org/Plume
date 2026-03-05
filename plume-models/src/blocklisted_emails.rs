use crate::{schema::email_blocklist, Connection, Error, Result};
use diesel::{self, ExpressionMethods, QueryDsl, RunQueryDsl, TextExpressionMethods};
use glob::Pattern;

#[derive(Clone, Queryable, Identifiable)]
#[diesel(table_name = email_blocklist)]
pub struct BlocklistedEmail {
    pub id: i32,
    pub email_address: String,
    pub note: String,
    pub notify_user: bool,
    pub notification_text: String,
}

#[derive(Insertable, FromForm)]
#[diesel(table_name = email_blocklist)]
pub struct NewBlocklistedEmail {
    pub email_address: String,
    pub note: String,
    pub notify_user: bool,
    pub notification_text: String,
}

impl BlocklistedEmail {
    insert!(email_blocklist, NewBlocklistedEmail);
    get!(email_blocklist);
    find_by!(email_blocklist, find_by_id, id as &i32);
    pub fn delete_entries(conn: &mut Connection, ids: &Vec<i32>) -> Result<bool> {
        use diesel::delete;
        for i in ids {
            let be: BlocklistedEmail = BlocklistedEmail::find_by_id(conn, i)?;
            delete(&be).execute(conn)?;
        }
        Ok(true)
    }
    pub fn find_for_domain(conn: &mut Connection, domain: &str) -> Result<Vec<BlocklistedEmail>> {
        let effective = format!("%@{}", domain);
        email_blocklist::table
            .filter(email_blocklist::email_address.like(effective))
            .load::<BlocklistedEmail>(conn)
            .map_err(Error::from)
    }
    pub fn matches_blocklist(conn: &mut Connection, email: &str) -> Result<Option<BlocklistedEmail>> {
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
    pub fn page(conn: &mut Connection, (min, max): (i32, i32)) -> Result<Vec<BlocklistedEmail>> {
        email_blocklist::table
            .offset(min.into())
            .limit((max - min).into())
            .load::<BlocklistedEmail>(conn)
            .map_err(Error::from)
    }
    pub fn count(conn: &mut Connection) -> Result<i64> {
        email_blocklist::table
            .count()
            .get_result(conn)
            .map_err(Error::from)
    }
    pub fn pattern_errors(pat: &str) -> Option<glob::PatternError> {
        let c = Pattern::new(pat);
        c.err()
    }
    pub fn new(
        conn: &mut Connection,
        pattern: &str,
        note: &str,
        show_notification: bool,
        notification_text: &str,
    ) -> Result<BlocklistedEmail> {
        let c = NewBlocklistedEmail {
            email_address: pattern.to_owned(),
            note: note.to_owned(),
            notify_user: show_notification,
            notification_text: notification_text.to_owned(),
        };
        BlocklistedEmail::insert(conn, c)
    }
}
#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::{instance::tests as instance_tests, tests::db, Connection as Conn};
    use diesel::Connection;

    pub(crate) fn fill_database(conn: &Conn) -> Vec<BlocklistedEmail> {
        instance_tests::fill_database(conn);
        let domainblock =
            BlocklistedEmail::new(conn, "*@bad-actor.com", "Mean spammers", false, "").unwrap();
        let userblock = BlocklistedEmail::new(
            conn,
            "spammer@lax-administration.com",
            "Decent enough domain, but this user is a problem.",
            true,
            "Stop it please",
        )
        .unwrap();
        vec![domainblock, userblock]
    }
    #[test]
    fn test_match() {
        let conn = db();
        conn.test_transaction::<_, (), _>(|| {
            let various = fill_database(&conn);
            let match1 = "user1@bad-actor.com";
            let match2 = "spammer@lax-administration.com";
            let no_match = "happy-user@lax-administration.com";
            assert_eq!(
                BlocklistedEmail::matches_blocklist(&conn, match1)
                    .unwrap()
                    .unwrap()
                    .id,
                various[0].id
            );
            assert_eq!(
                BlocklistedEmail::matches_blocklist(&conn, match2)
                    .unwrap()
                    .unwrap()
                    .id,
                various[1].id
            );
            assert!(BlocklistedEmail::matches_blocklist(&conn, no_match)
                .unwrap()
                .is_none());
            Ok(())
        });
    }
}
