use diesel::{self, ExpressionMethods, QueryDsl, RunQueryDsl};

use blogs::Blog;
use schema::{blogs, list_elems, lists, users};
use std::convert::{TryFrom, TryInto};
use users::User;
use {Connection, Error, Result};

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
struct NewList<'a> {
    pub name: &'a str,
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
struct NewListElem<'a> {
    pub list_id: i32,
    pub user_id: Option<i32>,
    pub blog_id: Option<i32>,
    pub word: Option<&'a str>,
}

impl List {
    last!(lists);
    get!(lists);

    fn insert(conn: &Connection, val: NewList) -> Result<Self> {
        diesel::insert_into(lists::table)
            .values(val)
            .execute(conn)?;
        List::last(conn)
    }

    pub fn list_for_user(conn: &Connection, user_id: Option<i32>) -> Result<Vec<Self>> {
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

    pub fn find_by_name(conn: &Connection, user_id: Option<i32>, name: &str) -> Result<Self> {
        if let Some(user_id) = user_id {
            lists::table
                .filter(lists::user_id.eq(user_id))
                .filter(lists::name.eq(name))
                .limit(1)
                .load::<Self>(conn)?
                .into_iter()
                .next()
                .ok_or(Error::NotFound)
        } else {
            lists::table
                .filter(lists::user_id.is_null())
                .filter(lists::name.eq(name))
                .limit(1)
                .load::<Self>(conn)?
                .into_iter()
                .next()
                .ok_or(Error::NotFound)
        }
    }

    pub fn new(conn: &Connection, name: &str, user: Option<&User>, kind: ListType) -> Result<Self> {
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
        self.type_
            .try_into()
            .expect("invalide list was constructed")
    }

    /// Return Ok(true) if the list contain the given user, Ok(false) otherwiser,
    /// and Err(_) on error
    pub fn contains_user(&self, conn: &Connection, user: i32) -> Result<bool> {
        private::ListElem::user_in_list(conn, self, user)
    }

    /// Return Ok(true) if the list contain the given blog, Ok(false) otherwiser,
    /// and Err(_) on error
    pub fn contains_blog(&self, conn: &Connection, blog: i32) -> Result<bool> {
        private::ListElem::blog_in_list(conn, self, blog)
    }

    /// Return Ok(true) if the list contain the given word, Ok(false) otherwiser,
    /// and Err(_) on error
    pub fn contains_word(&self, conn: &Connection, word: &str) -> Result<bool> {
        private::ListElem::word_in_list(conn, self, word)
    }

    /// Return Ok(true) if the list match the given prefix, Ok(false) otherwiser,
    /// and Err(_) on error
    pub fn contains_prefix(&self, conn: &Connection, word: &str) -> Result<bool> {
        private::ListElem::prefix_in_list(conn, self, word)
    }

    /// Insert new users in a list
    /// returns Ok(false) if this list isn't for users
    pub fn add_users(&self, conn: &Connection, users: &[i32]) -> Result<bool> {
        if self.kind() != ListType::User {
            return Ok(false);
        }
        diesel::insert_into(list_elems::table)
            .values(
                users
                    .iter()
                    .map(|u| NewListElem {
                        list_id: self.id,
                        user_id: Some(*u),
                        blog_id: None,
                        word: None,
                    })
                    .collect::<Vec<_>>(),
            )
            .execute(conn)?;
        Ok(true)
    }

    /// Insert new blogs in a list
    /// returns Ok(false) if this list isn't for blog
    pub fn add_blogs(&self, conn: &Connection, blogs: &[i32]) -> Result<bool> {
        if self.kind() != ListType::Blog {
            return Ok(false);
        }
        diesel::insert_into(list_elems::table)
            .values(
                blogs
                    .iter()
                    .map(|b| NewListElem {
                        list_id: self.id,
                        user_id: None,
                        blog_id: Some(*b),
                        word: None,
                    })
                    .collect::<Vec<_>>(),
            )
            .execute(conn)?;
        Ok(true)
    }

    /// Insert new words in a list
    /// returns Ok(false) if this list isn't for words
    pub fn add_words(&self, conn: &Connection, words: &[&str]) -> Result<bool> {
        if self.kind() != ListType::Word {
            return Ok(false);
        }
        diesel::insert_into(list_elems::table)
            .values(
                words
                    .iter()
                    .map(|w| NewListElem {
                        list_id: self.id,
                        user_id: None,
                        blog_id: None,
                        word: Some(w),
                    })
                    .collect::<Vec<_>>(),
            )
            .execute(conn)?;
        Ok(true)
    }

    /// Insert new prefixes in a list
    /// returns Ok(false) if this list isn't for prefix
    pub fn add_prefixes(&self, conn: &Connection, prefixes: &[&str]) -> Result<bool> {
        if self.kind() != ListType::Prefix {
            return Ok(false);
        }
        diesel::insert_into(list_elems::table)
            .values(
                prefixes
                    .iter()
                    .map(|p| NewListElem {
                        list_id: self.id,
                        user_id: None,
                        blog_id: None,
                        word: Some(p),
                    })
                    .collect::<Vec<_>>(),
            )
            .execute(conn)?;
        Ok(true)
    }

    /// Get all users in the list
    pub fn list_users(&self, conn: &Connection) -> Result<Vec<User>> {
        list_elems::table
            .filter(list_elems::list_id.eq(self.id))
            .inner_join(users::table)
            .select(users::all_columns)
            .load(conn)
            .map_err(Error::from)
    }

    /// Get all blogs in the list
    pub fn list_blogs(&self, conn: &Connection) -> Result<Vec<Blog>> {
        list_elems::table
            .filter(list_elems::list_id.eq(self.id))
            .inner_join(blogs::table)
            .select(blogs::all_columns)
            .load(conn)
            .map_err(Error::from)
    }

    /// Get all words in the list
    pub fn list_words(&self, conn: &Connection) -> Result<Vec<String>> {
        if self.kind() != ListType::Word {
            return Ok(vec![]);
        }
        list_elems::table
            .filter(list_elems::list_id.eq(self.id))
            .filter(list_elems::word.is_not_null())
            .select(list_elems::word)
            .load::<Option<String>>(conn)
            .map_err(Error::from)
            .map(|r| r.into_iter().filter_map(|o| o).collect::<Vec<String>>())
    }

    /// Get all prefixes in the list
    pub fn list_prefixes(&self, conn: &Connection) -> Result<Vec<String>> {
        if self.kind() != ListType::Prefix {
            return Ok(vec![]);
        }
        list_elems::table
            .filter(list_elems::list_id.eq(self.id))
            .filter(list_elems::word.is_not_null())
            .select(list_elems::word)
            .load::<Option<String>>(conn)
            .map_err(Error::from)
            .map(|r| r.into_iter().filter_map(|o| o).collect::<Vec<String>>())
    }

    pub fn clear(&self, conn: &Connection) -> Result<()> {
        diesel::delete(list_elems::table.filter(list_elems::list_id.eq(self.id)))
            .execute(conn)
            .map(|_| ())
            .map_err(Error::from)
    }

    pub fn set_users(&self, conn: &Connection, users: &[i32]) -> Result<bool> {
        if self.kind() != ListType::User {
            return Ok(false);
        }
        self.clear(conn)?;
        self.add_users(conn, users)
    }

    pub fn set_blogs(&self, conn: &Connection, blogs: &[i32]) -> Result<bool> {
        if self.kind() != ListType::Blog {
            return Ok(false);
        }
        self.clear(conn)?;
        self.add_blogs(conn, blogs)
    }

    pub fn set_words(&self, conn: &Connection, words: &[&str]) -> Result<bool> {
        if self.kind() != ListType::Word {
            return Ok(false);
        }
        self.clear(conn)?;
        self.add_words(conn, words)
    }

    pub fn set_prefixes(&self, conn: &Connection, prefixes: &[&str]) -> Result<bool> {
        if self.kind() != ListType::Prefix {
            return Ok(false);
        }
        self.clear(conn)?;
        self.add_prefixes(conn, prefixes)
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

#[cfg(test)]
mod tests {
    use super::*;
    use blogs::tests as blog_tests;
    use tests::db;

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
            &List::find_by_name(conn, l1.user_id, &l1.name).unwrap(),
        );
        l_eq(
            &&l1u,
            &List::find_by_name(conn, l1u.user_id, &l1u.name).unwrap(),
        );
    }

    #[test]
    fn test_user_list() {
        let conn = &db();
        let (users, blogs) = blog_tests::fill_database(conn);

        let l = List::new(conn, "list", None, ListType::User).unwrap();

        assert_eq!(l.kind(), ListType::User);
        assert!(l.list_users(conn).unwrap().is_empty());

        assert!(!l.contains_user(conn, users[0].id).unwrap());
        assert!(l.add_users(conn, &[users[0].id]).unwrap());
        assert!(l.contains_user(conn, users[0].id).unwrap());

        assert!(l.add_users(conn, &[users[1].id]).unwrap());
        assert!(l.contains_user(conn, users[0].id).unwrap());
        assert!(l.contains_user(conn, users[1].id).unwrap());
        assert_eq!(2, l.list_users(conn).unwrap().len());

        assert!(l.set_users(conn, &[users[0].id]).unwrap());
        assert!(l.contains_user(conn, users[0].id).unwrap());
        assert!(!l.contains_user(conn, users[1].id).unwrap());
        assert_eq!(1, l.list_users(conn).unwrap().len());
        assert!(users[0] == l.list_users(conn).unwrap()[0]);

        l.clear(conn).unwrap();
        assert!(l.list_users(conn).unwrap().is_empty());

        assert!(!l.add_blogs(conn, &[blogs[0].id]).unwrap());
    }

    #[test]
    fn test_blog_list() {
        let conn = &db();
        let (users, blogs) = blog_tests::fill_database(conn);

        let l = List::new(conn, "list", None, ListType::Blog).unwrap();

        assert_eq!(l.kind(), ListType::Blog);
        assert!(l.list_blogs(conn).unwrap().is_empty());

        assert!(!l.contains_blog(conn, blogs[0].id).unwrap());
        assert!(l.add_blogs(conn, &[blogs[0].id]).unwrap());
        assert!(l.contains_blog(conn, blogs[0].id).unwrap());

        assert!(l.add_blogs(conn, &[blogs[1].id]).unwrap());
        assert!(l.contains_blog(conn, blogs[0].id).unwrap());
        assert!(l.contains_blog(conn, blogs[1].id).unwrap());
        assert_eq!(2, l.list_blogs(conn).unwrap().len());

        assert!(l.set_blogs(conn, &[blogs[0].id]).unwrap());
        assert!(l.contains_blog(conn, blogs[0].id).unwrap());
        assert!(!l.contains_blog(conn, blogs[1].id).unwrap());
        assert_eq!(1, l.list_blogs(conn).unwrap().len());
        assert_eq!(blogs[0].id, l.list_blogs(conn).unwrap()[0].id);

        l.clear(conn).unwrap();
        assert!(l.list_blogs(conn).unwrap().is_empty());

        assert!(!l.add_users(conn, &[users[0].id]).unwrap());
    }

    #[test]
    fn test_word_list() {
        let conn = &db();

        let l = List::new(conn, "list", None, ListType::Word).unwrap();

        assert_eq!(l.kind(), ListType::Word);
        assert!(l.list_words(conn).unwrap().is_empty());

        assert!(!l.contains_word(conn, "plume").unwrap());
        assert!(l.add_words(conn, &["plume"]).unwrap());
        assert!(l.contains_word(conn, "plume").unwrap());
        assert!(!l.contains_word(conn, "plumelin").unwrap());

        assert!(l.add_words(conn, &["amsterdam"]).unwrap());
        assert!(l.contains_word(conn, "plume").unwrap());
        assert!(l.contains_word(conn, "amsterdam").unwrap());
        assert_eq!(2, l.list_words(conn).unwrap().len());

        assert!(l.set_words(conn, &["plume"]).unwrap());
        assert!(l.contains_word(conn, "plume").unwrap());
        assert!(!l.contains_word(conn, "amsterdam").unwrap());
        assert_eq!(1, l.list_words(conn).unwrap().len());
        assert_eq!("plume", l.list_words(conn).unwrap()[0]);

        l.clear(conn).unwrap();
        assert!(l.list_words(conn).unwrap().is_empty());

        assert!(!l.add_prefixes(conn, &["something"]).unwrap());
    }

    #[test]
    fn test_prefix_list() {
        let conn = &db();

        let l = List::new(conn, "list", None, ListType::Prefix).unwrap();

        assert_eq!(l.kind(), ListType::Prefix);
        assert!(l.list_prefixes(conn).unwrap().is_empty());

        assert!(!l.contains_prefix(conn, "plume").unwrap());
        assert!(l.add_prefixes(conn, &["plume"]).unwrap());
        assert!(l.contains_prefix(conn, "plume").unwrap());
        assert!(l.contains_prefix(conn, "plumelin").unwrap());

        assert!(l.add_prefixes(conn, &["amsterdam"]).unwrap());
        assert!(l.contains_prefix(conn, "plume").unwrap());
        assert!(l.contains_prefix(conn, "amsterdam").unwrap());
        assert_eq!(2, l.list_prefixes(conn).unwrap().len());

        assert!(l.set_prefixes(conn, &["plume"]).unwrap());
        assert!(l.contains_prefix(conn, "plume").unwrap());
        assert!(!l.contains_prefix(conn, "amsterdam").unwrap());
        assert_eq!(1, l.list_prefixes(conn).unwrap().len());
        assert_eq!("plume", l.list_prefixes(conn).unwrap()[0]);

        l.clear(conn).unwrap();
        assert!(l.list_prefixes(conn).unwrap().is_empty());

        assert!(!l.add_words(conn, &["something"]).unwrap());
    }

}
