use diesel::{self, ExpressionMethods, RunQueryDsl, QueryDsl};

use plume_common::activity_pub::Hashtag;
use {ap_url, Connection};
use instance::Instance;
use schema::tags;

#[derive(Serialize, Queryable, Clone)]
pub struct Tag {
    pub id: i32,
    pub tag: String,
    pub is_hastag: bool,
    pub post_id: i32
}

#[derive(Insertable)]
#[table_name = "tags"]
pub struct NewTag {
    pub tag: String,
    pub is_hastag: bool,
    pub post_id: i32
}

impl Tag {
    insert!(tags, NewTag);
    get!(tags);
    find_by!(tags, find_by_name, tag as String);
    list_by!(tags, for_post, post_id as i32);

    pub fn into_activity(&self, conn: &Connection) -> Hashtag {
        let mut ht = Hashtag::default();
        ht.set_href_string(ap_url(format!("{}/tag/{}", Instance::get_local(conn).unwrap().public_domain, self.tag))).expect("Tag::into_activity: href error");
        ht.set_name_string(self.tag.clone()).expect("Tag::into_activity: name error");
        ht
    }

    pub fn from_activity(conn: &Connection, tag: Hashtag, post: i32) -> Tag {
        Tag::insert(conn, NewTag {
            tag: tag.name_string().expect("Tag::from_activity: name error"),
            is_hastag: false,
            post_id: post
        })
    }
}
