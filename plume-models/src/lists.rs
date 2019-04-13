use diesel::{self, ExpressionMethods, QueryDsl, RunQueryDsl};

use schema::{list_elems, lists};
use std::convert::{TryFrom, TryInto};
use users::User;
use {Connection, Error, Result};

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ListType {
    User,
    Blog,
    Word,
    Prefix,
}

impl TryFrom<i32> for ListType {
    type Error = ();

    fn try_from(i: i32) -> std::result::Result<Self, ()> {
        match i {
            0 => Ok(ListType::User),
            1 => Ok(ListType::Blog),
            2 => Ok(ListType::Word),
            3 => Ok(ListType::Prefix),
            _ => Err(()),
        }
    }
}

impl Into<i32> for ListType {
    fn into(self) -> i32 {
        match self {
            ListType::User => 0,
            ListType::Blog => 1,
            ListType::Word => 2,
            ListType::Prefix => 3,
        }
    }
}

#[derive(Clone, Queryable, Identifiable)]
pub struct List {
    pub id: i32,
    pub name: String,
    pub user_id: Option<i32>,
    type_: i32,
}

#[derive(Default, Insertable)]
#[table_name = "lists"]
struct NewList {
    pub name: String,
    pub user_id: Option<i32>,
    type_: i32,
}

#[derive(Clone, Queryable, Identifiable)]
struct ListElem {
    pub id: i32,
    pub list_id: i32,
    pub user_id: Option<i32>,
    pub blog_id: Option<i32>,
    pub word: Option<String>,
}

#[derive(Default, Insertable)]
#[table_name = "list_elems"]
struct NewListElem {
    pub list_id: i32,
    pub user_id: Option<i32>,
    pub blog_id: Option<i32>,
    pub word: Option<String>,
}

impl List {
    fn insert(conn: &Connection, val: NewList) -> Result<Self> {
        diesel::insert_into(lists::table)
            .values(val)
            .execute(conn)?;
        List::last(conn)
    }
    last!(lists);
    get!(lists);
    list_by!(lists, list_for_user, user_id as Option<i32>);
    find_by!(lists, find_by_name, user_id as Option<i32>, name as &str);

    pub fn new(
        conn: &Connection,
        name: String,
        user: Option<&User>,
        kind: ListType,
    ) -> Result<Self> {
        Self::insert(
            conn,
            NewList {
                name,
                user_id: user.map(|u| u.id),
                type_: kind.into(),
            },
        )
    }

    pub fn kind(&self) -> ListType {
        self.type_
            .try_into()
            .expect("invalide list was constructed")
    }

    pub fn contains_user(&self, conn: &Connection, user: i32) -> Result<bool> {
        private::ListElem::user_in_list(conn, self, user)
    }

    pub fn contains_blog(&self, conn: &Connection, blog: i32) -> Result<bool> {
        private::ListElem::user_in_list(conn, self, blog)
    }

    pub fn contains_word(&self, conn: &Connection, word: &str) -> Result<bool> {
        private::ListElem::word_in_list(conn, self, word)
    }

    pub fn contains_prefix(&self, conn: &Connection, word: &str) -> Result<bool> {
        private::ListElem::prefix_in_list(conn, self, word)
    }

    /// returns Ok(false) if this list isn't for users
    pub fn add_users(&self, conn: &Connection, user: &[i32]) -> Result<bool> {
        if self.kind() != ListType::User {
            return Ok(false);
        }
        let _ = (conn, user);
        unimplemented!();
    }

    /// returns Ok(false) if this list isn't for blog
    pub fn add_blogs(&self, conn: &Connection, blog: &[i32]) -> Result<bool> {
        if self.kind() != ListType::Blog {
            return Ok(false);
        }
        let _ = (conn, blog);
        unimplemented!();
    }

    /// returns Ok(false) if this list isn't for words
    pub fn add_words(&self, conn: &Connection, word: &[&str]) -> Result<bool> {
        if self.kind() != ListType::Word {
            return Ok(false);
        }
        let _ = (conn, word);
        unimplemented!();
    }

    /// returns Ok(false) if this list isn't for prefix
    pub fn add_prefixs(&self, conn: &Connection, prefix: &[&str]) -> Result<bool> {
        if self.kind() != ListType::Prefix {
            return Ok(false);
        }
        let _ = (conn, prefix);
        unimplemented!();
    }
}

pub(super) mod private {
    pub use super::*;
    use diesel::{
        dsl,
        sql_types::{Nullable, Text},
        IntoSql, TextExpressionMethods,
    };

    impl ListElem {
        insert!(list_elems, NewListElem);
        list_by!(list_elems, for_list, list_id as i32);

        pub fn user_in_list(conn: &Connection, list: &List, user: i32) -> Result<bool> {
            dsl::select(dsl::exists(
                list_elems::table
                    .filter(list_elems::list_id.eq(list.id))
                    .filter(list_elems::user_id.eq(Some(user))),
            ))
            .get_result(conn)
            .map_err(Error::from)
        }

        pub fn blog_in_list(conn: &Connection, list: &List, blog: i32) -> Result<bool> {
            dsl::select(dsl::exists(
                list_elems::table
                    .filter(list_elems::list_id.eq(list.id))
                    .filter(list_elems::blog_id.eq(Some(blog))),
            ))
            .get_result(conn)
            .map_err(Error::from)
        }

        pub fn word_in_list(conn: &Connection, list: &List, word: &str) -> Result<bool> {
            dsl::select(dsl::exists(
                list_elems::table
                    .filter(list_elems::list_id.eq(list.id))
                    .filter(list_elems::word.eq(word)),
            ))
            .get_result(conn)
            .map_err(Error::from)
        }

        pub fn prefix_in_list(conn: &Connection, list: &List, word: &str) -> Result<bool> {
            dsl::select(dsl::exists(
                list_elems::table
                    .filter(
                        word.into_sql::<Nullable<Text>>()
                            .like(list_elems::word.concat("%")),
                    )
                    .filter(list_elems::list_id.eq(list.id)),
            ))
            .get_result(conn)
            .map_err(Error::from)
        }
    }
}
