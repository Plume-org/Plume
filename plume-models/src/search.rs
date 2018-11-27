use posts::Post;
use tags::Tag;
use mentions::Mention;
use Connection;

use chrono::Datelike;
use itertools::Itertools;
use tantivy::{
    collector::TopCollector,
    directory::MmapDirectory,
    query::QueryParser,
    schema::*,
    tokenizer::*,
    Index, IndexWriter, Term
};
use whatlang::{detect as detect_lang, Lang};
use std::{
    cmp,
    fs::create_dir_all,
    path::Path,
    sync::Mutex,
};

#[derive(Debug)]
pub enum SearcherError{
    IndexCreationError,
    WriteLockAcquisitionError,
    IndexOpeningError,
    IndexEditionError,
}

pub struct Searcher {
    index: Index,
    writer: Mutex<Option<IndexWriter>>,
}

impl Searcher {
    pub fn create(path: &AsRef<Path>) -> Result<Self,SearcherError> {
        let content_tokenizer = SimpleTokenizer
            .filter(RemoveLongFilter::limit(40))
            .filter(LowerCaser);

        let property_tokenizer = NgramTokenizer::new(2, 8, false)
            .filter(LowerCaser);

        let tag_indexing = TextOptions::default()
            .set_indexing_options(TextFieldIndexing::default()
                                  .set_tokenizer("content_tokenizer")
                                  .set_index_option(IndexRecordOption::Basic));

        let content_indexing = TextOptions::default()
            .set_indexing_options(TextFieldIndexing::default()
                                  .set_tokenizer("content_tokenizer")
                                  .set_index_option(IndexRecordOption::WithFreqsAndPositions));

        let property_indexing = TextOptions::default()
            .set_indexing_options(TextFieldIndexing::default()
                                  .set_tokenizer("property_tokenizer")
                                  .set_index_option(IndexRecordOption::WithFreqsAndPositions));

        let mut schema_builder = SchemaBuilder::default();

        schema_builder.add_i64_field("post_id", INT_STORED | INT_INDEXED);
        schema_builder.add_i64_field("creation_date", INT_INDEXED);
        schema_builder.add_i64_field("instance", INT_INDEXED);

        schema_builder.add_text_field("author", tag_indexing.clone());//todo move to a user_indexing with user_tokenizer function
        schema_builder.add_text_field("hashtag", tag_indexing.clone());
        schema_builder.add_text_field("mention", tag_indexing);

        schema_builder.add_text_field("blog_name", content_indexing.clone());
        schema_builder.add_text_field("content", content_indexing.clone());
        schema_builder.add_text_field("subtitle", content_indexing.clone());
        schema_builder.add_text_field("title", content_indexing);

        schema_builder.add_text_field("lang", property_indexing.clone());
        schema_builder.add_text_field("license", property_indexing);

        let schema = schema_builder.build();

        create_dir_all(path).map_err(|_| SearcherError::IndexCreationError)?;
        let index = Index::create(MmapDirectory::open(path).map_err(|_| SearcherError::IndexCreationError)?, schema).map_err(|_| SearcherError::IndexCreationError)?;

        {
            let tokenizer_manager = index.tokenizers();
            tokenizer_manager.register("content_tokenizer", content_tokenizer);
            tokenizer_manager.register("property_tokenizer", property_tokenizer);
        }//to please the borrow checker
        Ok(Self {
            writer: Mutex::new(Some(index.writer(50_000_000).map_err(|_| SearcherError::WriteLockAcquisitionError)?)),
            index
        })
    }

    pub fn open(path: &AsRef<Path>) -> Result<Self, SearcherError> {
        let content_tokenizer = SimpleTokenizer
            .filter(RemoveLongFilter::limit(40))
            .filter(LowerCaser);

        let property_tokenizer = NgramTokenizer::new(2, 8, false)
            .filter(LowerCaser);

        let index = Index::open(MmapDirectory::open(path).map_err(|_| SearcherError::IndexOpeningError)?).map_err(|_| SearcherError::IndexOpeningError)?;

        {
            let tokenizer_manager = index.tokenizers();
            tokenizer_manager.register("content_tokenizer", content_tokenizer);
            tokenizer_manager.register("property_tokenizer", property_tokenizer);
        }//to please the borrow checker
        let mut writer = index.writer(50_000_000).map_err(|_| SearcherError::WriteLockAcquisitionError)?;
        writer.garbage_collect_files().map_err(|_| SearcherError::IndexEditionError)?;
        Ok(Self {
            writer: Mutex::new(Some(writer)),
            index,
        })
    }

    pub fn add_document(&self, conn: &Connection, post: &Post) {
        let schema = self.index.schema();

        let post_id = schema.get_field("post_id").unwrap();
        let creation_date = schema.get_field("creation_date").unwrap();
        let instance = schema.get_field("instance").unwrap();

        let author = schema.get_field("author").unwrap();
        let hashtag = schema.get_field("hashtag").unwrap();
        let mention = schema.get_field("mention").unwrap();

        let blog_name = schema.get_field("blog_name").unwrap();
        let content = schema.get_field("content").unwrap();
        let subtitle = schema.get_field("subtitle").unwrap();
        let title = schema.get_field("title").unwrap();

        let lang = schema.get_field("lang").unwrap();
        let license = schema.get_field("license").unwrap();

        let mut writer = self.writer.lock().unwrap();
        let writer = writer.as_mut().unwrap();
        writer.add_document(doc!(
                post_id => i64::from(post.id),
                author => post.get_authors(conn).into_iter().map(|u| u.get_fqn(conn)).join(" "),
                creation_date => i64::from(post.creation_date.num_days_from_ce()),
                instance => i64::from(post.get_blog(conn).instance_id),
                hashtag => Tag::for_post(conn, post.id).into_iter().map(|t| t.tag).join(" "),
                mention => Mention::list_for_post(conn, post.id).into_iter().filter_map(|m| m.get_mentioned(conn))
                        .map(|u| u.get_fqn(conn)).join(" "),
                blog_name => post.get_blog(conn).title,
                content => post.content.get().clone(),
                subtitle => post.subtitle.clone(),
                title => post.title.clone(),
                lang => detect_lang(post.content.get()).and_then(|i| if i.is_reliable() { Some(i.lang()) } else {None} ).unwrap_or(Lang::Eng).name(),
                license => post.license.clone(),
                ));
    }

    pub fn delete_document(&self, post: &Post) {
        let schema = self.index.schema();
        let post_id = schema.get_field("post_id").unwrap();

        let doc_id = Term::from_field_i64(post_id, i64::from(post.id));
        let mut writer = self.writer.lock().unwrap();
        let writer = writer.as_mut().unwrap();
        writer.delete_term(doc_id);
    }

    pub fn update_document(&self, conn: &Connection, post: &Post) {
        self.delete_document(post);
        self.add_document(conn, post);
    }

    pub fn search_document(&self, conn: &Connection, query: &str, (min, max): (i32, i32)) -> Vec<Post>{
        let schema = self.index.schema();
        let post_id = schema.get_field("post_id").unwrap();

        let content = schema.get_field("content").unwrap();
        let subtitle = schema.get_field("subtitle").unwrap();
        let title = schema.get_field("title").unwrap();

        let query = QueryParser::for_index(&self.index, vec![content, subtitle, title])
            .parse_query(query).unwrap();

        let mut collector = TopCollector::with_limit(cmp::max(1,max) as usize);

        let searcher = self.index.searcher();
        searcher.search(&*query, &mut collector).unwrap();

        collector.docs().get(min as usize..).unwrap_or(&[])
            .into_iter()
            .filter_map(|doc_add| {
                let doc = searcher.doc(*doc_add).ok()?;
                let id = doc.get_first(post_id)?;
                Post::get(conn, id.i64_value() as i32)
                    //borrow checker don't want me to use filter_map or and_then here
                          })
            .collect()
    }

    pub fn commit(&self) {
        let mut writer = self.writer.lock().unwrap();
        writer.as_mut().unwrap().commit().unwrap();
        self.index.load_searchers().unwrap();
    }

    pub fn drop_writer(&self) {
        self.writer.lock().unwrap().take();
    }
}


#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::env::temp_dir;
    use diesel::Connection;

    use plume_common::activity_pub::inbox::Deletable;
    use plume_common::utils::random_hex;
    use blogs::tests::fill_database;
    use posts::NewPost;
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
            assert_eq!(searcher.search_document(conn, &title, (0,1))[0].id, post.id);

            let newtitle = random_hex()[..8].to_owned();
            post.title = newtitle.clone();
            post.update(conn, &searcher);
            searcher.commit();
            assert_eq!(searcher.search_document(conn, &newtitle, (0,1))[0].id, post.id);
            assert!(searcher.search_document(conn, &title, (0,1)).is_empty());

            post.delete(&(conn, &searcher));
            searcher.commit();
            assert!(searcher.search_document(conn, &newtitle, (0,1)).is_empty());

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
