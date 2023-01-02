pub mod actor;
mod query;
mod searcher;
mod tokenizer;
pub use self::query::PlumeQuery as Query;
pub use self::searcher::*;
pub use self::tokenizer::TokenizerKind;

#[cfg(test)]
pub(crate) mod tests {
    use super::{Query, Searcher};
    use crate::{
        blogs::tests::fill_database,
        config::SearchTokenizerConfig,
        post_authors::*,
        posts::{NewPost, Post},
        safe_string::SafeString,
        tests::db,
        CONFIG,
    };
    use diesel::Connection;
    use plume_common::utils::random_hex;
    use std::env::temp_dir;
    use std::str::FromStr;

    pub(crate) fn get_searcher(tokenizers: &SearchTokenizerConfig) -> Searcher {
        let dir = temp_dir().join(&format!("plume-test-{}", random_hex()));
        if dir.exists() {
            Searcher::open(&dir, tokenizers)
        } else {
            Searcher::create(&dir, tokenizers)
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
        let dir = temp_dir().join(format!("plume-test-{}", random_hex()));
        {
            Searcher::create(&dir, &CONFIG.search_tokenizers).unwrap();
        }
        Searcher::open(&dir, &CONFIG.search_tokenizers).unwrap();
    }

    #[test]
    fn create() {
        let dir = temp_dir().join(format!("plume-test-{}", random_hex()));

        assert!(Searcher::open(&dir, &CONFIG.search_tokenizers).is_err());
        {
            Searcher::create(&dir, &CONFIG.search_tokenizers).unwrap();
        }
        Searcher::open(&dir, &CONFIG.search_tokenizers).unwrap(); //verify it's well created
    }

    #[test]
    fn search() {
        let conn = &db();
        conn.test_transaction::<_, (), _>(|| {
            let searcher = get_searcher(&CONFIG.search_tokenizers);
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
            searcher.add_document(conn, &post).unwrap();
            searcher.commit();
            assert_eq!(
                searcher.search_document(conn, Query::from_str(&title).unwrap(), (0, 1))[0].id,
                post.id
            );

            let newtitle = random_hex()[..8].to_owned();
            post.title = newtitle.clone();
            post.update(conn).unwrap();
            searcher.update_document(conn, &post).unwrap();
            searcher.commit();
            assert_eq!(
                searcher.search_document(conn, Query::from_str(&newtitle).unwrap(), (0, 1))[0].id,
                post.id
            );
            assert!(searcher
                .search_document(conn, Query::from_str(&title).unwrap(), (0, 1))
                .is_empty());

            searcher.delete_document(&post);
            searcher.commit();
            assert!(searcher
                .search_document(conn, Query::from_str(&newtitle).unwrap(), (0, 1))
                .is_empty());
            Ok(())
        });
    }

    #[cfg(feature = "search-lindera")]
    #[test]
    fn search_japanese() {
        let conn = &db();
        conn.test_transaction::<_, (), _>(|| {
            let tokenizers = SearchTokenizerConfig {
                tag_tokenizer: TokenizerKind::Lindera,
                content_tokenizer: TokenizerKind::Lindera,
                property_tokenizer: TokenizerKind::Ngram,
            };
            let searcher = get_searcher(&tokenizers);
            let blog = &fill_database(conn).1[0];

            let title = random_hex()[..8].to_owned();

            let post = Post::insert(
                conn,
                NewPost {
                    blog_id: blog.id,
                    slug: title.clone(),
                    title: title.clone(),
                    content: SafeString::new("ブログエンジンPlumeです。"),
                    published: true,
                    license: "CC-BY-SA".to_owned(),
                    ap_url: "".to_owned(),
                    creation_date: None,
                    subtitle: "".to_owned(),
                    source: "".to_owned(),
                    cover_id: None,
                },
            )
            .unwrap();

            searcher.commit();

            assert_eq!(
                searcher.search_document(conn, Query::from_str("ブログエンジン").unwrap(), (0, 1))
                    [0]
                .id,
                post.id
            );
            assert_eq!(
                searcher.search_document(conn, Query::from_str("Plume").unwrap(), (0, 1))[0].id,
                post.id
            );
            assert_eq!(
                searcher.search_document(conn, Query::from_str("です").unwrap(), (0, 1))[0].id,
                post.id
            );
            assert_eq!(
                searcher.search_document(conn, Query::from_str("。").unwrap(), (0, 1))[0].id,
                post.id
            );

            Ok(())
        });
    }

    #[test]
    fn drop_writer() {
        let searcher = get_searcher(&CONFIG.search_tokenizers);
        searcher.drop_writer();
        get_searcher(&CONFIG.search_tokenizers);
    }
}
