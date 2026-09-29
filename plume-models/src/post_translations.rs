use chrono::NaiveDateTime;
use diesel::{self, ExpressionMethods, QueryDsl, RunQueryDsl};

use crate::{
    posts::Post,
    schema::post_translations,
    Connection, Error, Result,
};

#[derive(Clone, Identifiable, Queryable, Debug)]
pub struct PostTranslation {
    pub id: i32,
    pub post_id: i32,
    pub source_lang: String,
    pub target_lang: String,
    pub title: String,
    pub subtitle: String,
    pub content: String,
    pub source: String,
    pub creation_date: NaiveDateTime,
}

#[derive(Insertable)]
#[table_name = "post_translations"]
pub struct NewPostTranslation {
    pub post_id: i32,
    pub source_lang: String,
    pub target_lang: String,
    pub title: String,
    pub subtitle: String,
    pub content: String,
    pub source: String,
}

impl PostTranslation {
    get!(post_translations);
    list_by!(post_translations, for_post, post_id as i32);
    list_by!(post_translations, for_lang, target_lang as &str);

    find_by!(
        post_translations,
        find_by_post_and_lang,
        post_id as i32,
        target_lang as &str
    );

    pub fn insert(conn: &Connection, new: NewPostTranslation) -> Result<Self> {
        diesel::insert_into(post_translations::table)
            .values(new)
            .execute(conn)?;
        Self::last(conn)
    }

    last!(post_translations);

    pub fn delete(&self, conn: &Connection) -> Result<()> {
        diesel::delete(self)
            .execute(conn)
            .map(|_| ())
            .map_err(Error::from)
    }

    pub fn get_post(&self, conn: &Connection) -> Result<Post> {
        Post::get(conn, self.post_id)
    }
}
