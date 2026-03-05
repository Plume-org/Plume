use crate::{schema::password_reset_requests, Connection, Error, Result};
use chrono::{offset::Utc, Duration, NaiveDateTime};
use diesel::{ExpressionMethods, QueryDsl, RunQueryDsl};

#[derive(Clone, Identifiable, Queryable)]
pub struct PasswordResetRequest {
    pub id: i32,
    pub email: String,
    pub token: String,
    pub expiration_date: NaiveDateTime,
}

#[derive(Insertable)]
#[diesel(table_name = password_reset_requests)]
pub struct NewPasswordResetRequest {
    pub email: String,
    pub token: String,
    pub expiration_date: NaiveDateTime,
}

const TOKEN_VALIDITY_HOURS: i64 = 2;

impl PasswordResetRequest {
    pub fn insert(conn: &mut Connection, email: &str) -> Result<String> {
        // first, delete other password reset tokens associated with this email:
        let existing_requests =
            password_reset_requests::table.filter(password_reset_requests::email.eq(email));
        diesel::delete(existing_requests).execute(conn)?;

        // now, generate a random token, set the expiry date,
        // and insert it into the DB:
        let token = plume_common::utils::random_hex();
        let expiration_date = Utc::now()
            .naive_utc()
            .checked_add_signed(Duration::hours(TOKEN_VALIDITY_HOURS))
            .expect("could not calculate expiration date");
        let new_request = NewPasswordResetRequest {
            email: email.to_owned(),
            token: token.clone(),
            expiration_date,
        };
        diesel::insert_into(password_reset_requests::table)
            .values(new_request)
            .execute(conn)
            .map_err(Error::from)?;

        Ok(token)
    }

    pub fn find_by_token(conn: &mut Connection, token: &str) -> Result<Self> {
        let token = password_reset_requests::table
            .filter(password_reset_requests::token.eq(token))
            .first::<Self>(conn)
            .map_err(Error::from)?;

        if token.expiration_date < Utc::now().naive_utc() {
            return Err(Error::Expired);
        }

        Ok(token)
    }

    pub fn find_and_delete_by_token(conn: &mut Connection, token: &str) -> Result<Self> {
        let request = Self::find_by_token(conn, token)?;

        let filter =
            password_reset_requests::table.filter(password_reset_requests::id.eq(request.id));
        diesel::delete(filter).execute(conn)?;

        Ok(request)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{tests::db, users::tests as user_tests};
    use diesel::Connection;

    #[test]
    fn test_insert_and_find_password_reset_request() {
        let conn = db();
        conn.test_transaction::<_, (), _>(|| {
            user_tests::fill_database(&conn);
            let admin_email = "admin@example.com";

            let token = PasswordResetRequest::insert(&conn, admin_email)
                .expect("couldn't insert new request");
            let request = PasswordResetRequest::find_by_token(&conn, &token)
                .expect("couldn't retrieve request");

            assert!(token.len() > 32);
            assert_eq!(&request.email, &admin_email);

            Ok(())
        });
    }

    #[test]
    fn test_insert_delete_previous_password_reset_request() {
        let conn = db();
        conn.test_transaction::<_, (), _>(|| {
            user_tests::fill_database(&conn);
            let admin_email = "admin@example.com";

            PasswordResetRequest::insert(&conn, admin_email).expect("couldn't insert new request");
            PasswordResetRequest::insert(&conn, admin_email)
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
            let admin_email = "admin@example.com";
            let token = "abcdef";
            let now = Utc::now().naive_utc();

            diesel::insert_into(password_reset_requests::table)
                .values((
                    password_reset_requests::email.eq(&admin_email),
                    password_reset_requests::token.eq(&token),
                    password_reset_requests::expiration_date.eq(now),
                ))
                .execute(&*conn)
                .expect("could not insert request");

            match PasswordResetRequest::find_by_token(&conn, token) {
                Err(Error::Expired) => (),
                _ => panic!("Received unexpected result finding expired token"),
            }

            Ok(())
        });
    }

    #[test]
    fn test_find_and_delete_password_reset_request() {
        let conn = db();
        conn.test_transaction::<_, (), _>(|| {
            user_tests::fill_database(&conn);
            let admin_email = "admin@example.com";

            let token = PasswordResetRequest::insert(&conn, admin_email)
                .expect("couldn't insert new request");
            PasswordResetRequest::find_and_delete_by_token(&conn, &token)
                .expect("couldn't find and delete request");

            let count = password_reset_requests::table.count().get_result(&*conn);
            assert_eq!(Ok(0), count);

            Ok(())
        });
    }
}
