use crate::{
    db_conn::DbConn,
    schema::email_signups,
    users::{NewUser, Role, User},
    Error, Result,
};
use chrono::{offset::Utc, Duration, NaiveDateTime};
use diesel::{
    Connection as _, ExpressionMethods, Identifiable, Insertable, QueryDsl, Queryable, RunQueryDsl,
};
use plume_common::utils::random_hex;
use std::ops::Deref;

const TOKEN_VALIDITY_HOURS: i64 = 2;

#[repr(transparent)]
pub struct Token(String);

impl From<String> for Token {
    fn from(string: String) -> Self {
        Token(string)
    }
}

impl From<Token> for String {
    fn from(token: Token) -> Self {
        token.0
    }
}

impl Deref for Token {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Token {
    fn generate() -> Self {
        Self(random_hex())
    }
}

#[derive(Identifiable, Queryable)]
pub struct EmailSignup {
    pub id: i32,
    pub email: String,
    pub token: String,
    pub expiration_date: NaiveDateTime,
}

#[derive(Insertable)]
#[table_name = "email_signups"]
pub struct NewEmailSignup<'a> {
    pub email: &'a str,
    pub token: &'a str,
    pub expiration_date: NaiveDateTime,
}

impl EmailSignup {
    pub fn start(conn: &DbConn, email: &str) -> Result<Token> {
        conn.transaction(|| {
            Self::ensure_user_not_exist_by_email(conn, email)?;
            let _rows = Self::delete_existings_by_email(conn, email)?;
            let token = Token::generate();
            let expiration_date = Utc::now()
                .naive_utc()
                .checked_add_signed(Duration::hours(TOKEN_VALIDITY_HOURS))
                .expect("could not calculate expiration date");
            let new_signup = NewEmailSignup {
                email,
                token: &token,
                expiration_date,
            };
            let _rows = diesel::insert_into(email_signups::table)
                .values(new_signup)
                .execute(&**conn)?;

            Ok(token)
        })
    }

    pub fn confirm(conn: &DbConn, token: &Token, email: &str) -> Result<Self> {
        Self::ensure_user_not_exist_by_email(conn, email)?;
        let signup: Self = email_signups::table
            .filter(email_signups::token.eq(token.as_str()))
            .first(&**conn)
            .map_err(Error::from)?;
        if signup.expired() {
            Self::delete_existings_by_email(conn, email)?;
            return Err(Error::Expired);
        }
        Ok(signup)
    }

    pub fn complete(
        &self,
        conn: &DbConn,
        username: String,
        display_name: String,
        summary: &str,
        password: String,
    ) -> Result<User> {
        Self::ensure_user_not_exist_by_email(conn, &self.email)?;
        let user = NewUser::new_local(
            conn,
            username,
            display_name,
            Role::Normal,
            summary,
            self.email.clone(),
            Some(password),
        )?;
        self.delete(conn)?;
        Ok(user)
    }

    fn delete(&self, conn: &DbConn) -> Result<()> {
        let _rows = diesel::delete(self).execute(&**conn).map_err(Error::from)?;
        Ok(())
    }

    fn ensure_user_not_exist_by_email(conn: &DbConn, email: &str) -> Result<()> {
        if User::email_used(conn, email)? {
            let _rows = Self::delete_existings_by_email(conn, email)?;
            return Err(Error::UserAlreadyExists);
        }
        Ok(())
    }

    fn delete_existings_by_email(conn: &DbConn, email: &str) -> Result<usize> {
        let existing_signups = email_signups::table.filter(email_signups::email.eq(email));
        diesel::delete(existing_signups)
            .execute(&**conn)
            .map_err(Error::from)
    }

    fn expired(&self) -> bool {
        self.expiration_date < Utc::now().naive_utc()
    }
}
