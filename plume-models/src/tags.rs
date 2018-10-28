use diesel::{self, ExpressionMethods, RunQueryDsl, QueryDsl};

use plume_common::activity_pub::Hashtag;
use {ap_url, Connection};
use instance::Instance;
use schema::tags;

#[derive(Clone, Identifiable, Serialize, Queryable)]
pub struct Tag {
    pub id: i32,
    pub tag: String,
    pub is_hashtag: bool,
    pub post_id: i32
}

#[derive(Insertable)]
#[table_name = "tags"]
pub struct NewTag {
    pub tag: String,
    pub is_hashtag: bool,
    pub post_id: i32
}

impl Tag {
    insert!(tags, NewTag);
    get!(tags);
    find_by!(tags, find_by_name, tag as String);
    list_by!(tags, for_post, post_id as i32);

    pub fn into_activity(&self, conn: &Connection) -> Hashtag {
        let mut ht = Hashtag::default();
        ht.set_href_string(ap_url(format!("{}/tag/{}",
                                          Instance::get_local(conn).expect("Tag::into_activity: local instance not found error").public_domain,
                                          self.tag)
                                  )).expect("Tag::into_activity: href error");
        ht.set_name_string(self.tag.clone()).expect("Tag::into_activity: name error");
        ht
    }

    pub fn from_activity(conn: &Connection, tag: Hashtag, post: i32, is_hashtag: bool) -> Tag {
        Tag::insert(conn, NewTag {
            tag: tag.name_string().expect("Tag::from_activity: name error"),
            is_hashtag,
            post_id: post
        })
    }

    pub fn build_activity(conn: &Connection, tag: String) -> Hashtag {
        let mut ht = Hashtag::default();
        ht.set_href_string(ap_url(format!("{}/tag/{}",
                                          Instance::get_local(conn).expect("Tag::into_activity: local instance not found error").public_domain,
                                          tag)
                                  )).expect("Tag::into_activity: href error");
        ht.set_name_string(tag).expect("Tag::into_activity: name error");
        ht
    }

    pub fn delete(&self, conn: &Connection) {
        diesel::delete(self).execute(conn).expect("Tag::delete: database error");
    }
}
