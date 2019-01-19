use diesel::{self, ExpressionMethods, QueryDsl, RunQueryDsl};

use instance::Instance;
use plume_common::activity_pub::Hashtag;
use schema::tags;
use {ap_url, Connection, Error, Result};

#[derive(Clone, Identifiable, Serialize, Queryable)]
pub struct Tag {
    pub id: i32,
    pub tag: String,
    pub is_hashtag: bool,
    pub post_id: i32,
}

#[derive(Insertable)]
#[table_name = "tags"]
pub struct NewTag {
    pub tag: String,
    pub is_hashtag: bool,
    pub post_id: i32,
}

impl Tag {
    insert!(tags, NewTag);
    get!(tags);
    find_by!(tags, find_by_name, tag as &str);
    list_by!(tags, for_post, post_id as i32);

    pub fn to_activity(&self, conn: &Connection) -> Result<Hashtag> {
        let mut ht = Hashtag::default();
        ht.set_href_string(ap_url(&format!(
            "{}/tag/{}",
            Instance::get_local(conn)?.public_domain,
            self.tag
        )))?;
        ht.set_name_string(self.tag.clone())?;
        Ok(ht)
    }

    pub fn from_activity(conn: &Connection, tag: &Hashtag, post: i32, is_hashtag: bool) -> Result<Tag> {
        Tag::insert(
            conn,
            NewTag {
                tag: tag.name_string()?,
                is_hashtag,
                post_id: post,
            },
        )
    }

    pub fn build_activity(conn: &Connection, tag: String) -> Result<Hashtag> {
        let mut ht = Hashtag::default();
        ht.set_href_string(ap_url(&format!(
            "{}/tag/{}",
            Instance::get_local(conn)?.public_domain,
            tag
        )))?;
        ht.set_name_string(tag)?;
        Ok(ht)
    }

    pub fn delete(&self, conn: &Connection) -> Result<()> {
        diesel::delete(self)
            .execute(conn)
            .map(|_| ())
            .map_err(Error::from)
    }
}
