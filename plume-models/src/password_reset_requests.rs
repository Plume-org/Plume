use chrono::NaiveDateTime;
use diesel::{ExpressionMethods, QueryDsl, RunQueryDsl};
use diesel::dsl::{now, IntervalDsl};
use schema::password_reset_requests;
use {Connection, Error, Result};

#[derive(Clone, Identifiable, Queryable)]
pub struct PasswordResetRequest {
    pub id: i32,
    pub email: String,
    pub token: String,
    pub creation_date: NaiveDateTime,
}

#[derive(Insertable)]
#[table_name = "password_reset_requests"]
pub struct NewPasswordResetRequest {
    pub email: String,
    pub token: String,
}

impl PasswordResetRequest {
    pub fn insert(conn: &Connection, email: &str) -> Result<Self> {
        // first, delete other password reset tokens associated with this email:
        let existing_requests = password_reset_requests::table
            .filter(password_reset_requests::email.eq(email));
        diesel::delete(existing_requests).execute(conn)?;

        // now, generate a random token, insert in the DB, and return it:
        let token = plume_common::utils::random_hex();
        let new_request = NewPasswordResetRequest {
            email: email.to_owned(),
            token: token,
        };
        diesel::insert_into(password_reset_requests::table)
            .values(new_request)
            .get_result(conn)
            .map_err(Error::from)
    }

    pub fn find_by_token(conn: &Connection, token: &str) -> Result<Self> {
        password_reset_requests::table
            .filter(password_reset_requests::token.eq(token))
            .filter(password_reset_requests::creation_date.gt(now - 2.hours()))
            .first::<Self>(conn)
            .map_err(Error::from)
    }

    pub fn find_and_delete_by_token(conn: &Connection, token: &str) -> Result<Self> {
        let request = Self::find_by_token(&conn, &token)?;

        let filter = password_reset_requests::table
            .filter(password_reset_requests::id.eq(request.id));
        diesel::delete(filter).execute(conn)?;

        Ok(request)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use diesel::Connection;
    use tests::db;
    use users::tests as user_tests;

    #[test]
    fn test_insert_and_find_password_reset_request() {
        let conn = db();
        conn.test_transaction::<_, (), _>(|| {
            user_tests::fill_database(&conn);
            let admin_email = "admin@example.com";

            let request = PasswordResetRequest::insert(&conn, admin_email)
                .expect("couldn't insert new request");
            let request2 = PasswordResetRequest::find_by_token(&conn, &request.token)
                .expect("couldn't retrieve request");

            assert!(&request.token.len() > &32);
            assert_eq!(&request2.email, &admin_email);

            Ok(())
        });
    }

    #[test]
    fn test_insert_delete_previous_password_reset_request() {
        let conn = db();
        conn.test_transaction::<_, (), _>(|| {
            user_tests::fill_database(&conn);
            let admin_email = "admin@example.com";

            PasswordResetRequest::insert(&conn, &admin_email)
                .expect("couldn't insert new request");
            PasswordResetRequest::insert(&conn, &admin_email)
                .expect("couldn't insert second request");

            let count = password_reset_requests::table.count().get_result(&*conn);
            assert_eq!(Ok(1), count);

            Ok(())
        });
    }

    #[test]
    fn test_find_password_reset_request_by_token_time() {
        let conn = db();
        conn.test_transaction::<_, (), _>(|| {
            user_tests::fill_database(&conn);

            // TODO: best way to test this?
            Ok(())
        });
    }

    #[test]
    fn test_find_and_delete_password_reset_request() {
        let conn = db();
        conn.test_transaction::<_, (), _>(|| {
            user_tests::fill_database(&conn);
            let admin_email = "admin@example.com";

            let request = PasswordResetRequest::insert(&conn, &admin_email)
                .expect("couldn't insert new request");
            PasswordResetRequest::find_and_delete_by_token(&conn, &request.token)
                .expect("couldn't find and delete request");

            let count = password_reset_requests::table.count().get_result(&*conn);
            assert_eq!(Ok(0), count);

            Ok(())
        });
    }
}
