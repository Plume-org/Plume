use crate::{
    blogs::Blog,
    schema::{blogs, list_elems, lists, users},
    users::User,
    Connection, Error, Result,
};
use diesel::{self, ExpressionMethods, QueryDsl, RunQueryDsl};
use std::convert::{TryFrom, TryInto};

/// Represent what a list is supposed to store. Represented in database as an integer
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

impl From<ListType> for i32 {
    fn from(list_type: ListType) -> Self {
        match list_type {
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
#[diesel(table_name = lists)]
struct NewList<'a> {
    pub name: &'a str,
    pub user_id: Option<i32>,
    type_: i32,
}

macro_rules! func {
    (@elem User $id:expr, $value:expr) => {
        NewListElem {
            list_id: $id,
            user_id: Some(*$value),
            blog_id: None,
            word: None,
        }
    };
    (@elem Blog $id:expr, $value:expr) => {
        NewListElem {
            list_id: $id,
            user_id: None,
            blog_id: Some(*$value),
            word: None,
        }
    };
    (@elem Word $id:expr, $value:expr) => {
        NewListElem {
            list_id: $id,
            user_id: None,
            blog_id: None,
            word: Some($value),
        }
    };
    (@elem Prefix $id:expr, $value:expr) => {
        NewListElem {
            list_id: $id,
            user_id: None,
            blog_id: None,
            word: Some($value),
        }
    };
    (@in_type User) => { i32 };
    (@in_type Blog) => { i32 };
    (@in_type Word) => { &str };
    (@in_type Prefix) => { &str };
    (@out_type User) => { User };
    (@out_type Blog) => { Blog };
    (@out_type Word) => { String };
    (@out_type Prefix) => { String };

    (add: $fn:ident, $kind:ident) => {
        pub fn $fn(&self, conn: &mut Connection, vals: &[func!(@in_type $kind)]) -> Result<()> {
            if self.kind() != ListType::$kind {
                return Err(Error::InvalidValue);
            }
            diesel::insert_into(list_elems::table)
                .values(
                    vals
                        .iter()
                        .map(|u| func!(@elem $kind self.id, u))
                        .collect::<Vec<_>>(),
                )
                .execute(conn)?;
            Ok(())
        }
    };

    (list: $fn:ident, $kind:ident, $table:ident) => {
        pub fn $fn(&self, conn: &mut Connection) -> Result<Vec<func!(@out_type $kind)>> {
            if self.kind() != ListType::$kind {
                return Err(Error::InvalidValue);
            }
            list_elems::table
                .filter(list_elems::list_id.eq(self.id))
                .inner_join($table::table)
                .select($table::all_columns)
                .load(conn)
                .map_err(Error::from)
        }
    };



    (set: $fn:ident, $kind:ident, $add:ident) => {
        pub fn $fn(&self, conn: &mut Connection, val: &[func!(@in_type $kind)]) -> Result<()> {
            if self.kind() != ListType::$kind {
                return Err(Error::InvalidValue);
            }
            self.clear(conn)?;
            self.$add(conn, val)
        }
    }
}

#[allow(dead_code)]
#[derive(Clone, Queryable, Identifiable)]
struct ListElem {
    pub id: i32,
    pub list_id: i32,
    pub user_id: Option<i32>,
    pub blog_id: Option<i32>,
    pub word: Option<String>,
}

#[derive(Default, Insertable)]
#[diesel(table_name = list_elems)]
struct NewListElem<'a> {
    pub list_id: i32,
    pub user_id: Option<i32>,
    pub blog_id: Option<i32>,
    pub word: Option<&'a str>,
}

impl List {
    last!(lists);
    get!(lists);

    fn insert(conn: &mut Connection, val: NewList<'_>) -> Result<Self> {
        diesel::insert_into(lists::table)
            .values(val)
            .execute(conn)?;
        List::last(conn)
    }

    pub fn list_for_user(conn: &mut Connection, user_id: Option<i32>) -> Result<Vec<Self>> {
        if let Some(user_id) = user_id {
            lists::table
                .filter(lists::user_id.eq(user_id))
                .load::<Self>(conn)
                .map_err(Error::from)
        } else {
            lists::table
                .filter(lists::user_id.is_null())
                .load::<Self>(conn)
                .map_err(Error::from)
        }
    }

    pub fn find_for_user_by_name(
        conn: &mut Connection,
        user_id: Option<i32>,
        name: &str,
    ) -> Result<Self> {
        if let Some(user_id) = user_id {
            lists::table
                .filter(lists::user_id.eq(user_id))
                .filter(lists::name.eq(name))
                .first(conn)
                .map_err(Error::from)
        } else {
            lists::table
                .filter(lists::user_id.is_null())
                .filter(lists::name.eq(name))
                .first(conn)
                .map_err(Error::from)
        }
    }

    pub fn new(conn: &mut Connection, name: &str, user: Option<&User>, kind: ListType) -> Result<Self> {
        Self::insert(
            conn,
            NewList {
                name,
                user_id: user.map(|u| u.id),
                type_: kind.into(),
            },
        )
    }

    /// Returns the kind of a list
    pub fn kind(&self) -> ListType {
        self.type_.try_into().expect("invalid list was constructed")
    }

    /// Return Ok(true) if the list contain the given user, Ok(false) otherwiser,
    /// and Err(_) on error
    pub fn contains_user(&self, conn: &mut Connection, user: i32) -> Result<bool> {
        private::ListElem::user_in_list(conn, self, user)
    }

    /// Return Ok(true) if the list contain the given blog, Ok(false) otherwiser,
    /// and Err(_) on error
    pub fn contains_blog(&self, conn: &mut Connection, blog: i32) -> Result<bool> {
        private::ListElem::blog_in_list(conn, self, blog)
    }

    /// Return Ok(true) if the list contain the given word, Ok(false) otherwiser,
    /// and Err(_) on error
    pub fn contains_word(&self, conn: &mut Connection, word: &str) -> Result<bool> {
        private::ListElem::word_in_list(conn, self, word)
    }

    /// Return Ok(true) if the list match the given prefix, Ok(false) otherwiser,
    /// and Err(_) on error
    pub fn contains_prefix(&self, conn: &mut Connection, word: &str) -> Result<bool> {
        private::ListElem::prefix_in_list(conn, self, word)
    }

    // Insert new users in a list
    func! {add: add_users, User}

    // Insert new blogs in a list
    func! {add: add_blogs, Blog}

    // Insert new words in a list
    func! {add: add_words, Word}

    // Insert new prefixes in a list
    func! {add: add_prefixes, Prefix}

    // Get all users in the list
    func! {list: list_users, User, users}

    // Get all blogs in the list
    func! {list: list_blogs, Blog, blogs}

    /// Get all words in the list
    pub fn list_words(&self, conn: &mut Connection) -> Result<Vec<String>> {
        self.list_stringlike(conn, ListType::Word)
    }

    /// Get all prefixes in the list
    pub fn list_prefixes(&self, conn: &mut Connection) -> Result<Vec<String>> {
        self.list_stringlike(conn, ListType::Prefix)
    }

    #[inline(always)]
    fn list_stringlike(&self, conn: &mut Connection, t: ListType) -> Result<Vec<String>> {
        if self.kind() != t {
            return Err(Error::InvalidValue);
        }
        list_elems::table
            .filter(list_elems::list_id.eq(self.id))
            .filter(list_elems::word.is_not_null())
            .select(list_elems::word)
            .load::<Option<String>>(conn)
            .map_err(Error::from)
            // .map(|r| r.into_iter().filter_map(|o| o).collect::<Vec<String>>())
            .map(|r| r.into_iter().flatten().collect::<Vec<String>>())
    }

    pub fn clear(&self, conn: &mut Connection) -> Result<()> {
        diesel::delete(list_elems::table.filter(list_elems::list_id.eq(self.id)))
            .execute(conn)
            .map(|_| ())
            .map_err(Error::from)
    }

    pub fn delete(&self, conn: &mut Connection) -> Result<()> {
        if let Some(user_id) = self.user_id {
            diesel::delete(
                lists::table
                    .filter(lists::user_id.eq(user_id))
                    .filter(lists::name.eq(&self.name)),
            )
            .execute(conn)
            .map(|_| ())
            .map_err(Error::from)
        } else {
            diesel::delete(
                lists::table
                    .filter(lists::user_id.is_null())
                    .filter(lists::name.eq(&self.name)),
            )
            .execute(conn)
            .map(|_| ())
            .map_err(Error::from)
        }
    }

    func! {set: set_users, User, add_users}
    func! {set: set_blogs, Blog, add_blogs}
    func! {set: set_words, Word, add_words}
    func! {set: set_prefixes, Prefix, add_prefixes}
}

mod private {
    pub use super::*;
    use diesel::{
        dsl,
        sql_types::{Nullable, Text},
        IntoSql, TextExpressionMethods,
    };

    impl ListElem {
        insert!(list_elems, NewListElem<'_>);

        pub fn user_in_list(conn: &mut Connection, list: &List, user: i32) -> Result<bool> {
            dsl::select(dsl::exists(
                list_elems::table
                    .filter(list_elems::list_id.eq(list.id))
                    .filter(list_elems::user_id.eq(Some(user))),
            ))
            .get_result(conn)
            .map_err(Error::from)
        }

        pub fn blog_in_list(conn: &mut Connection, list: &List, blog: i32) -> Result<bool> {
            dsl::select(dsl::exists(
                list_elems::table
                    .filter(list_elems::list_id.eq(list.id))
                    .filter(list_elems::blog_id.eq(Some(blog))),
            ))
            .get_result(conn)
            .map_err(Error::from)
        }

        pub fn word_in_list(conn: &mut Connection, list: &List, word: &str) -> Result<bool> {
            dsl::select(dsl::exists(
                list_elems::table
                    .filter(list_elems::list_id.eq(list.id))
                    .filter(list_elems::word.eq(word)),
            ))
            .get_result(conn)
            .map_err(Error::from)
        }

        pub fn prefix_in_list(conn: &mut Connection, list: &List, word: &str) -> Result<bool> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{blogs::tests as blog_tests, tests::db};
    use diesel::Connection;

    #[test]
    fn list_type() {
        for i in 0..4 {
            assert_eq!(i, Into::<i32>::into(ListType::try_from(i).unwrap()));
        }
        ListType::try_from(4).unwrap_err();
    }

    #[test]
    fn list_lists() {
        let conn = &db();
        conn.test_transaction::<_, (), _>(|| {
            let (users, _) = blog_tests::fill_database(conn);

            let l1 = List::new(conn, "list1", None, ListType::User).unwrap();
            let l2 = List::new(conn, "list2", None, ListType::Blog).unwrap();
            let l1u = List::new(conn, "list1", Some(&users[0]), ListType::Word).unwrap();

            let l_eq = |l1: &List, l2: &List| {
                assert_eq!(l1.id, l2.id);
                assert_eq!(l1.user_id, l2.user_id);
                assert_eq!(l1.name, l2.name);
                assert_eq!(l1.type_, l2.type_);
            };

            let l1bis = List::get(conn, l1.id).unwrap();
            l_eq(&l1, &l1bis);

            let l_inst = List::list_for_user(conn, None).unwrap();
            let l_user = List::list_for_user(conn, Some(users[0].id)).unwrap();
            assert_eq!(2, l_inst.len());
            assert_eq!(1, l_user.len());
            assert!(l_inst.iter().all(|l| l.id != l1u.id));

            l_eq(&l1u, &l_user[0]);
            if l_inst[0].id == l1.id {
                l_eq(&l1, &l_inst[0]);
                l_eq(&l2, &l_inst[1]);
            } else {
                l_eq(&l1, &l_inst[1]);
                l_eq(&l2, &l_inst[0]);
            }

            l_eq(
                &l1,
                &List::find_for_user_by_name(conn, l1.user_id, &l1.name).unwrap(),
            );
            l_eq(
                &l1u,
                &List::find_for_user_by_name(conn, l1u.user_id, &l1u.name).unwrap(),
            );
            Ok(())
        });
    }

    #[test]
    fn test_user_list() {
        let conn = &db();
        conn.test_transaction::<_, (), _>(|| {
            let (users, blogs) = blog_tests::fill_database(conn);

            let l = List::new(conn, "list", None, ListType::User).unwrap();

            assert_eq!(l.kind(), ListType::User);
            assert!(l.list_users(conn).unwrap().is_empty());

            assert!(!l.contains_user(conn, users[0].id).unwrap());
            assert!(l.add_users(conn, &[users[0].id]).is_ok());
            assert!(l.contains_user(conn, users[0].id).unwrap());

            assert!(l.add_users(conn, &[users[1].id]).is_ok());
            assert!(l.contains_user(conn, users[0].id).unwrap());
            assert!(l.contains_user(conn, users[1].id).unwrap());
            assert_eq!(2, l.list_users(conn).unwrap().len());

            assert!(l.set_users(conn, &[users[0].id]).is_ok());
            assert!(l.contains_user(conn, users[0].id).unwrap());
            assert!(!l.contains_user(conn, users[1].id).unwrap());
            assert_eq!(1, l.list_users(conn).unwrap().len());
            assert!(users[0] == l.list_users(conn).unwrap()[0]);

            l.clear(conn).unwrap();
            assert!(l.list_users(conn).unwrap().is_empty());

            assert!(l.add_blogs(conn, &[blogs[0].id]).is_err());
            Ok(())
        });
    }

    #[test]
    fn test_blog_list() {
        let conn = &db();
        conn.test_transaction::<_, (), _>(|| {
            let (users, blogs) = blog_tests::fill_database(conn);

            let l = List::new(conn, "list", None, ListType::Blog).unwrap();

            assert_eq!(l.kind(), ListType::Blog);
            assert!(l.list_blogs(conn).unwrap().is_empty());

            assert!(!l.contains_blog(conn, blogs[0].id).unwrap());
            assert!(l.add_blogs(conn, &[blogs[0].id]).is_ok());
            assert!(l.contains_blog(conn, blogs[0].id).unwrap());

            assert!(l.add_blogs(conn, &[blogs[1].id]).is_ok());
            assert!(l.contains_blog(conn, blogs[0].id).unwrap());
            assert!(l.contains_blog(conn, blogs[1].id).unwrap());
            assert_eq!(2, l.list_blogs(conn).unwrap().len());

            assert!(l.set_blogs(conn, &[blogs[0].id]).is_ok());
            assert!(l.contains_blog(conn, blogs[0].id).unwrap());
            assert!(!l.contains_blog(conn, blogs[1].id).unwrap());
            assert_eq!(1, l.list_blogs(conn).unwrap().len());
            assert_eq!(blogs[0].id, l.list_blogs(conn).unwrap()[0].id);

            l.clear(conn).unwrap();
            assert!(l.list_blogs(conn).unwrap().is_empty());

            assert!(l.add_users(conn, &[users[0].id]).is_err());
            Ok(())
        });
    }

    #[test]
    fn test_word_list() {
        let conn = &db();
        conn.test_transaction::<_, (), _>(|| {
            let l = List::new(conn, "list", None, ListType::Word).unwrap();

            assert_eq!(l.kind(), ListType::Word);
            assert!(l.list_words(conn).unwrap().is_empty());

            assert!(!l.contains_word(conn, "plume").unwrap());
            assert!(l.add_words(conn, &["plume"]).is_ok());
            assert!(l.contains_word(conn, "plume").unwrap());
            assert!(!l.contains_word(conn, "plumelin").unwrap());

            assert!(l.add_words(conn, &["amsterdam"]).is_ok());
            assert!(l.contains_word(conn, "plume").unwrap());
            assert!(l.contains_word(conn, "amsterdam").unwrap());
            assert_eq!(2, l.list_words(conn).unwrap().len());

            assert!(l.set_words(conn, &["plume"]).is_ok());
            assert!(l.contains_word(conn, "plume").unwrap());
            assert!(!l.contains_word(conn, "amsterdam").unwrap());
            assert_eq!(1, l.list_words(conn).unwrap().len());
            assert_eq!("plume", l.list_words(conn).unwrap()[0]);

            l.clear(conn).unwrap();
            assert!(l.list_words(conn).unwrap().is_empty());

            assert!(l.add_prefixes(conn, &["something"]).is_err());
            Ok(())
        });
    }

    #[test]
    fn test_prefix_list() {
        let conn = &db();
        conn.test_transaction::<_, (), _>(|| {
            let l = List::new(conn, "list", None, ListType::Prefix).unwrap();

            assert_eq!(l.kind(), ListType::Prefix);
            assert!(l.list_prefixes(conn).unwrap().is_empty());

            assert!(!l.contains_prefix(conn, "plume").unwrap());
            assert!(l.add_prefixes(conn, &["plume"]).is_ok());
            assert!(l.contains_prefix(conn, "plume").unwrap());
            assert!(l.contains_prefix(conn, "plumelin").unwrap());

            assert!(l.add_prefixes(conn, &["amsterdam"]).is_ok());
            assert!(l.contains_prefix(conn, "plume").unwrap());
            assert!(l.contains_prefix(conn, "amsterdam").unwrap());
            assert_eq!(2, l.list_prefixes(conn).unwrap().len());

            assert!(l.set_prefixes(conn, &["plume"]).is_ok());
            assert!(l.contains_prefix(conn, "plume").unwrap());
            assert!(!l.contains_prefix(conn, "amsterdam").unwrap());
            assert_eq!(1, l.list_prefixes(conn).unwrap().len());
            assert_eq!("plume", l.list_prefixes(conn).unwrap()[0]);

            l.clear(conn).unwrap();
            assert!(l.list_prefixes(conn).unwrap().is_empty());

            assert!(l.add_words(conn, &["something"]).is_err());
            Ok(())
        });
    }
}
