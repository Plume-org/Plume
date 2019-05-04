mod query;
mod searcher;
mod tokenizer;
pub use self::query::PlumeQuery as Query;
pub use self::searcher::*;

#[cfg(test)]
pub(crate) mod tests {
    use super::{Query, Searcher};
    use diesel::Connection;
    use std::env::temp_dir;
    use std::str::FromStr;

    use crate::blogs::tests::fill_database;
    use crate::post_authors::*;
    use crate::posts::{NewPost, Post};
    use crate::safe_string::SafeString;
    use crate::tests::db;
    use plume_common::utils::random_hex;

    pub(crate) fn get_searcher() -> Searcher {
        let dir = temp_dir().join("plume-test");
        if dir.exists() {
            Searcher::open(&dir)
        } else {
            Searcher::create(&dir)
        }
        .unwrap()
    }

    #[test]
    fn get_first_token() {
        let vector = vec![
            ("+\"my token\" other", ("+\"my token\"", " other")),
            ("-\"my token\" other", ("-\"my token\"", " other")),
            (" \"my token\" other", ("\"my token\"", " other")),
            ("\"my token\" other", ("\"my token\"", " other")),
            ("+my token other", ("+my", " token other")),
            ("-my token other", ("-my", " token other")),
            (" my token other", ("my", " token other")),
            ("my token other", ("my", " token other")),
            ("+\"my token other", ("+\"my token other", "")),
            ("-\"my token other", ("-\"my token other", "")),
            (" \"my token other", ("\"my token other", "")),
            ("\"my token other", ("\"my token other", "")),
        ];
        for (source, res) in vector {
            assert_eq!(Query::get_first_token(source), res);
        }
    }

    #[test]
    fn from_str() {
        let vector = vec![
            ("", ""),
            ("a query", "a query"),
            ("\"a query\"", "\"a query\""),
            ("+a -\"query\"", "+a -query"),
            ("title:\"something\" a query", "a query title:something"),
            ("-title:\"something\" a query", "a query -title:something"),
            ("author:user@domain", "author:user@domain"),
            ("-author:@user@domain", "-author:user@domain"),
            ("before:2017-11-05 before:2018-01-01", "before:2017-11-05"),
            ("after:2017-11-05 after:2018-01-01", "after:2018-01-01"),
        ];
        for (source, res) in vector {
            assert_eq!(&Query::from_str(source).unwrap().to_string(), res);
            assert_eq!(Query::new().parse_query(source).to_string(), res);
        }
    }

    #[test]
    fn setters() {
        let vector = vec![
            ("something", "title:something"),
            ("+something", "+title:something"),
            ("-something", "-title:something"),
            ("+\"something\"", "+title:something"),
            ("+some thing", "+title:\"some thing\""),
        ];
        for (source, res) in vector {
            assert_eq!(&Query::new().title(source, None).to_string(), res);
        }

        let vector = vec![
            ("something", "author:something"),
            ("+something", "+author:something"),
            ("-something", "-author:something"),
            ("+\"something\"", "+author:something"),
            ("+@someone@somewhere", "+author:someone@somewhere"),
        ];
        for (source, res) in vector {
            assert_eq!(&Query::new().author(source, None).to_string(), res);
        }
    }

    #[test]
    fn open() {
        {
            get_searcher()
        }; //make sure $tmp/plume-test-tantivy exist

        let dir = temp_dir().join("plume-test");
        Searcher::open(&dir).unwrap();
    }

    #[test]
    fn create() {
        let dir = temp_dir().join(format!("plume-test-{}", random_hex()));

        assert!(Searcher::open(&dir).is_err());
        {
            Searcher::create(&dir).unwrap();
        }
        Searcher::open(&dir).unwrap(); //verify it's well created
    }

    #[test]
    fn search() {
        let conn = &db();
        conn.test_transaction::<_, (), _>(|| {
            let searcher = get_searcher();
            let blog = &fill_database(conn).1[0];
            let author = &blog.list_authors(conn).unwrap()[0];

            let title = random_hex()[..8].to_owned();

            let mut post = Post::insert(
                conn,
                NewPost {
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
                },
                &searcher,
            )
            .unwrap();
            PostAuthor::insert(
                conn,
                NewPostAuthor {
                    post_id: post.id,
                    author_id: author.id,
                },
            )
            .unwrap();

            searcher.commit();
            assert_eq!(
                searcher.search_document(conn, Query::from_str(&title).unwrap(), (0, 1))[0].id,
                post.id
            );

            let newtitle = random_hex()[..8].to_owned();
            post.title = newtitle.clone();
            post.update(conn, &searcher).unwrap();
            searcher.commit();
            assert_eq!(
                searcher.search_document(conn, Query::from_str(&newtitle).unwrap(), (0, 1))[0].id,
                post.id
            );
            assert!(searcher
                .search_document(conn, Query::from_str(&title).unwrap(), (0, 1))
                .is_empty());

            post.delete(conn, &searcher).unwrap();
            searcher.commit();
            assert!(searcher
                .search_document(conn, Query::from_str(&newtitle).unwrap(), (0, 1))
                .is_empty());

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
