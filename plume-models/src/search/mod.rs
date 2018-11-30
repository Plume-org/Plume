mod searcher;
mod query;
mod tokenizer;
pub use self::searcher::*;
pub use self::query::PlumeQuery as Query;


#[cfg(test)]
pub(crate) mod tests {
    use super::{Query, Searcher};
    use std::env::temp_dir;
    use diesel::Connection;

    use plume_common::activity_pub::inbox::Deletable;
    use plume_common::utils::random_hex;
    use blogs::tests::fill_database;
    use posts::{NewPost, Post};
    use post_authors::*;
    use safe_string::SafeString;
    use tests::db;


    pub(crate) fn get_searcher() -> Searcher {
        let dir = temp_dir().join("plume-test");
        if dir.exists() {
            Searcher::open(&dir)
        } else {
            Searcher::create(&dir)
        }.unwrap()
    }

    #[test]
    fn get_first_token() {
        assert_eq!(Query::get_first_token("+\"my token\" other"), ("+\"my token\"", " other"));
        assert_eq!(Query::get_first_token("-\"my token\" other"), ("-\"my token\"", " other"));
        assert_eq!(Query::get_first_token(" \"my token\" other"), ("\"my token\"", " other"));
        assert_eq!(Query::get_first_token("\"my token\" other"), ("\"my token\"", " other"));
        assert_eq!(Query::get_first_token("+my token other"), ("+my", " token other"));
        assert_eq!(Query::get_first_token("-my token other"), ("-my", " token other"));
        assert_eq!(Query::get_first_token(" my token other"), ("my", " token other"));
        assert_eq!(Query::get_first_token("my token other"), ("my", " token other"));
        assert_eq!(Query::get_first_token("+\"my token other"), ("+\"my token other", ""));
        assert_eq!(Query::get_first_token("-\"my token other"), ("-\"my token other", ""));
        assert_eq!(Query::get_first_token(" \"my token other"), ("\"my token other", ""));
        assert_eq!(Query::get_first_token("\"my token other"), ("\"my token other", ""));
    }

    #[test]
    fn from_str() {
        assert_eq!(&Query::from_str("").to_string(), "");
        assert_eq!(&Query::from_str("a query").to_string(), "a query");
        assert_eq!(&Query::from_str("+a -\"query\"").to_string(), "+a -query");
        assert_eq!(&Query::from_str("title:\"something\" a query").to_string(), "a query title:something");
        assert_eq!(&Query::from_str("-title:\"something\" a query").to_string(), "a query -title:something");
        assert_eq!(&Query::from_str("author:user@domain").to_string(), "author:user@domain");
        assert_eq!(&Query::from_str("-author:@user@domain").to_string(), "-author:user@domain");
        assert_eq!(&Query::from_str("before:2017-11-05 before:2018-01-01").to_string(), "before:2017-11-05");
        assert_eq!(&Query::from_str("after:2017-11-05 after:2018-01-01").to_string(), "after:2018-01-01");
    }

    #[test]
    fn open() {
        {get_searcher()};//make sure $tmp/plume-test-tantivy exist

        let dir = temp_dir().join("plume-test");
        Searcher::open(&dir).unwrap();
    }

    #[test]
    fn create() {
        let dir = temp_dir().join(format!("plume-test-{}", random_hex()));

        assert!(Searcher::open(&dir).is_err());
        {Searcher::create(&dir).unwrap();}
        Searcher::open(&dir).unwrap();//verify it's well created
    }

    #[test]
    fn search() {
        let conn = &db();
        conn.test_transaction::<_, (), _>(|| {
            let searcher = get_searcher();
            let blog = &fill_database(conn)[0];
            let author = &blog.list_authors(conn)[0];

            let title = random_hex()[..8].to_owned();

            let mut post = Post::insert(conn, NewPost {
                blog_id: blog.id,
                slug: title.clone(),
                title: title.clone(),
                content: SafeString::new(""),
                published: true,
                license: "CC-BY-SA".to_owned(),
                ap_url: "".to_owned(),
                creation_date: None,
                subtitle: "".to_owned(),
                source: "".to_owned(),
                cover_id: None,
            }, &searcher);
            PostAuthor::insert(conn, NewPostAuthor {
                post_id: post.id,
                author_id: author.id,
            });

            searcher.commit();
            assert_eq!(searcher.search_document(conn, Query::from_str(&title), (0,1))[0].id, post.id);

            let newtitle = random_hex()[..8].to_owned();
            post.title = newtitle.clone();
            post.update(conn, &searcher);
            searcher.commit();
            assert_eq!(searcher.search_document(conn, Query::from_str(&newtitle), (0,1))[0].id, post.id);
            assert!(searcher.search_document(conn, Query::from_str(&title), (0,1)).is_empty());

            post.delete(&(conn, &searcher));
            searcher.commit();
            assert!(searcher.search_document(conn, Query::from_str(&newtitle), (0,1)).is_empty());

            Ok(())
        });
    }

    #[test]
    fn drop_writer() {
        let searcher = get_searcher();
        searcher.drop_writer();
        get_searcher();
    }
}
