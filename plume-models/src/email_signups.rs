use crate::{
    blocklisted_emails::BlocklistedEmail,
    Connection,
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
#[diesel(table_name = email_signups)]
pub struct NewEmailSignup<'a> {
    pub email: &'a str,
    pub token: &'a str,
    pub expiration_date: NaiveDateTime,
}

impl EmailSignup {
    pub fn start(conn: &mut DbConn, email: &str) -> Result<Token> {
        Self::ensure_email_not_blocked(conn, email)?;

        conn.transaction(|conn| {
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
                .execute(conn)?;

            Ok(token)
        })
    }

    pub fn find_by_token(conn: &mut DbConn, token: Token) -> Result<Self> {
        let signup = email_signups::table
            .filter(email_signups::token.eq(token.as_str()))
            .first::<Self>(&mut **conn)
            .map_err(Error::from)?;
        Ok(signup)
    }

    pub fn confirm(&self, conn: &mut DbConn) -> Result<()> {
        Self::ensure_email_not_blocked(conn, &self.email)?;

        conn.transaction(|conn| {
            Self::ensure_user_not_exist_by_email(conn, &self.email)?;
            if self.expired() {
                Self::delete_existings_by_email(conn, &self.email)?;
                return Err(Error::Expired);
            }
            Ok(())
        })
    }

    pub fn complete(&self, conn: &mut DbConn, username: String, password: String) -> Result<User> {
        Self::ensure_email_not_blocked(conn, &self.email)?;

        conn.transaction(|conn| {
            Self::ensure_user_not_exist_by_email(conn, &self.email)?;
            let user = NewUser::new_local(
                conn,
                username,
                "".to_string(),
                Role::Normal,
                "",
                self.email.clone(),
                Some(User::hash_pass(&password)?),
            )?;
            self.delete(conn)?;
            Ok(user)
        })
    }

    fn delete(&self, conn: &mut Connection) -> Result<()> {
        let _rows = diesel::delete(self).execute(conn).map_err(Error::from)?;
        Ok(())
    }

    fn ensure_email_not_blocked(conn: &mut DbConn, email: &str) -> Result<()> {
        if let Some(x) = BlocklistedEmail::matches_blocklist(conn, email)? {
            Err(Error::Blocklisted(x.notify_user, x.notification_text))
        } else {
            Ok(())
        }
    }

    fn ensure_user_not_exist_by_email(conn: &mut Connection, email: &str) -> Result<()> {
        if User::email_used(conn, email)? {
            let _rows = Self::delete_existings_by_email(conn, email)?;
            return Err(Error::UserAlreadyExists);
        }
        Ok(())
    }

    fn delete_existings_by_email(conn: &mut Connection, email: &str) -> Result<usize> {
        let existing_signups = email_signups::table.filter(email_signups::email.eq(email));
        diesel::delete(existing_signups)
            .execute(conn)
            .map_err(Error::from)
    }

    fn expired(&self) -> bool {
        self.expiration_date < Utc::now().naive_utc()
    }
}
