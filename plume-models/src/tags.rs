use crate::{ap_url, instance::Instance, schema::tags, Connection, Error, Result};
use activitystreams::iri_string::types::IriString;
use diesel::{self, ExpressionMethods, QueryDsl, RunQueryDsl};
use plume_common::activity_pub::{Hashtag, HashtagExt};

#[derive(Clone, Identifiable, Queryable)]
pub struct Tag {
    pub id: i32,
    pub tag: String,
    pub is_hashtag: bool,
    pub post_id: i32,
}

#[derive(Insertable)]
#[diesel(table_name = tags)]
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

    pub fn to_activity(&self) -> Result<Hashtag> {
        let mut ht = Hashtag::new();
        ht.set_href(
            ap_url(&format!(
                "{}/tag/{}",
                Instance::get_local()?.public_domain,
                self.tag
            ))
            .parse::<IriString>()?,
        );
        ht.set_name(self.tag.clone());
        Ok(ht)
    }

    pub fn from_activity(
        conn: &mut Connection,
        tag: &Hashtag,
        post: i32,
        is_hashtag: bool,
    ) -> Result<Tag> {
        Tag::insert(
            conn,
            NewTag {
                tag: tag.name().ok_or(Error::MissingApProperty)?.as_str().into(),
                is_hashtag,
                post_id: post,
            },
        )
    }

    pub fn build_activity(tag: String) -> Result<Hashtag> {
        let mut ht = Hashtag::new();
        ht.set_href(
            ap_url(&format!(
                "{}/tag/{}",
                Instance::get_local()?.public_domain,
                tag
            ))
            .parse::<IriString>()?,
        );
        ht.set_name(tag);
        Ok(ht)
    }

    pub fn delete(&self, conn: &mut Connection) -> Result<()> {
        diesel::delete(self)
            .execute(conn)
            .map(|_| ())
            .map_err(Error::from)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::db;
    use crate::{diesel::Connection, inbox::tests::fill_database};
    use assert_json_diff::assert_json_eq;
    use serde_json::to_value;

    #[test]
    fn from_activity() {
        let conn = &db();
        conn.test_transaction::<_, Error, _>(|| {
            let (posts, _users, _blogs) = fill_database(conn);
            let post_id = posts[0].id;
            let mut ht = Hashtag::new();
            ht.set_href(ap_url("https://plu.me/tag/a_tag").parse::<IriString>()?);
            ht.set_name("a_tag".to_string());
            let tag = Tag::from_activity(conn, &ht, post_id, true)?;

            assert_eq!(&tag.tag, "a_tag");
            assert!(tag.is_hashtag);

            Ok(())
        });
    }

    #[test]
    fn to_activity() {
        let conn = &db();
        conn.test_transaction::<_, Error, _>(|| {
            fill_database(conn);
            let tag = Tag {
                id: 0,
                tag: "a_tag".into(),
                is_hashtag: false,
                post_id: 0,
            };
            let act = tag.to_activity()?;
            let expected = json!({
                "href": "https://plu.me/tag/a_tag",
                "name": "a_tag",
                "type": "Hashtag"
            });

            assert_json_eq!(to_value(&act)?, expected);

            Ok(())
        })
    }

    #[test]
    fn build_activity() {
        let conn = &db();
        conn.test_transaction::<_, Error, _>(|| {
            fill_database(conn);
            let act = Tag::build_activity("a_tag".into())?;
            let expected = json!({
                "href": "https://plu.me/tag/a_tag",
                "name": "a_tag",
                "type": "Hashtag"
            });

            assert_json_eq!(to_value(&act)?, expected);

            Ok(())
        });
    }
}
