use crate::{
    instance::Instance,
    posts::Post,
    schema::posts,
    search::{query::PlumeQuery, tokenizer},
    tags::Tag,
    Connection, Result,
};
use chrono::Datelike;
use diesel::{ExpressionMethods, QueryDsl, RunQueryDsl};
use itertools::Itertools;
use std::{cmp, fs::create_dir_all, path::Path, sync::Mutex};
use tantivy::{
    collector::TopDocs, directory::MmapDirectory, schema::*, tokenizer::*, Index, IndexReader,
    IndexWriter, ReloadPolicy, Term,
};
use whatlang::{detect as detect_lang, Lang};

#[derive(Debug)]
pub enum SearcherError {
    IndexCreationError,
    WriteLockAcquisitionError,
    IndexOpeningError,
    IndexEditionError,
}

pub struct Searcher {
    index: Index,
    reader: IndexReader,
    writer: Mutex<Option<IndexWriter>>,
}

impl Searcher {
    pub fn schema() -> Schema {
        let tag_indexing = TextOptions::default().set_indexing_options(
            TextFieldIndexing::default()
                .set_tokenizer("whitespace_tokenizer")
                .set_index_option(IndexRecordOption::Basic),
        );

        let content_indexing = TextOptions::default().set_indexing_options(
            TextFieldIndexing::default()
                .set_tokenizer("content_tokenizer")
                .set_index_option(IndexRecordOption::WithFreqsAndPositions),
        );

        let property_indexing = TextOptions::default().set_indexing_options(
            TextFieldIndexing::default()
                .set_tokenizer("property_tokenizer")
                .set_index_option(IndexRecordOption::WithFreqsAndPositions),
        );

        let mut schema_builder = SchemaBuilder::default();

        schema_builder.add_i64_field("post_id", STORED | INDEXED);
        schema_builder.add_i64_field("creation_date", INDEXED);

        schema_builder.add_text_field("instance", tag_indexing.clone());
        schema_builder.add_text_field("author", tag_indexing.clone());
        schema_builder.add_text_field("tag", tag_indexing);

        schema_builder.add_text_field("blog", content_indexing.clone());
        schema_builder.add_text_field("content", content_indexing.clone());
        schema_builder.add_text_field("subtitle", content_indexing.clone());
        schema_builder.add_text_field("title", content_indexing);

        schema_builder.add_text_field("lang", property_indexing.clone());
        schema_builder.add_text_field("license", property_indexing);

        schema_builder.build()
    }

    pub fn create(path: &dyn AsRef<Path>) -> Result<Self> {
        let whitespace_tokenizer =
            TextAnalyzer::from(tokenizer::WhitespaceTokenizer).filter(LowerCaser);

        let content_tokenizer = TextAnalyzer::from(SimpleTokenizer)
            .filter(RemoveLongFilter::limit(40))
            .filter(LowerCaser);

        let property_tokenizer =
            TextAnalyzer::from(NgramTokenizer::new(2, 8, false)).filter(LowerCaser);

        let schema = Self::schema();

        create_dir_all(path).map_err(|_| SearcherError::IndexCreationError)?;
        let index = Index::create(
            MmapDirectory::open(path).map_err(|_| SearcherError::IndexCreationError)?,
            schema,
        )
        .map_err(|_| SearcherError::IndexCreationError)?;

        {
            let tokenizer_manager = index.tokenizers();
            tokenizer_manager.register("whitespace_tokenizer", whitespace_tokenizer);
            tokenizer_manager.register("content_tokenizer", content_tokenizer);
            tokenizer_manager.register("property_tokenizer", property_tokenizer);
        } //to please the borrow checker
        Ok(Self {
            writer: Mutex::new(Some(
                index
                    .writer(50_000_000)
                    .map_err(|_| SearcherError::WriteLockAcquisitionError)?,
            )),
            reader: index
                .reader_builder()
                .reload_policy(ReloadPolicy::Manual)
                .try_into()
                .map_err(|_| SearcherError::IndexCreationError)?,
            index,
        })
    }

    pub fn open(path: &dyn AsRef<Path>) -> Result<Self> {
        let whitespace_tokenizer =
            TextAnalyzer::from(tokenizer::WhitespaceTokenizer).filter(LowerCaser);

        let content_tokenizer = TextAnalyzer::from(SimpleTokenizer)
            .filter(RemoveLongFilter::limit(40))
            .filter(LowerCaser);

        let property_tokenizer =
            TextAnalyzer::from(NgramTokenizer::new(2, 8, false)).filter(LowerCaser);

        let index =
            Index::open(MmapDirectory::open(path).map_err(|_| SearcherError::IndexOpeningError)?)
                .map_err(|_| SearcherError::IndexOpeningError)?;

        {
            let tokenizer_manager = index.tokenizers();
            tokenizer_manager.register("whitespace_tokenizer", whitespace_tokenizer);
            tokenizer_manager.register("content_tokenizer", content_tokenizer);
            tokenizer_manager.register("property_tokenizer", property_tokenizer);
        } //to please the borrow checker
        let mut writer = index
            .writer(50_000_000)
            .map_err(|_| SearcherError::WriteLockAcquisitionError)?;
        writer
            .garbage_collect_files()
            .map_err(|_| SearcherError::IndexEditionError)?;
        Ok(Self {
            writer: Mutex::new(Some(writer)),
            reader: index
                .reader_builder()
                .reload_policy(ReloadPolicy::Manual)
                .try_into()
                .map_err(|_| SearcherError::IndexCreationError)?,
            index,
        })
    }

    pub fn add_document(&self, conn: &Connection, post: &Post) -> Result<()> {
        if !post.published {
            return Ok(());
        }

        let schema = self.index.schema();

        let post_id = schema.get_field("post_id").unwrap();
        let creation_date = schema.get_field("creation_date").unwrap();

        let instance = schema.get_field("instance").unwrap();
        let author = schema.get_field("author").unwrap();
        let tag = schema.get_field("tag").unwrap();

        let blog_name = schema.get_field("blog").unwrap();
        let content = schema.get_field("content").unwrap();
        let subtitle = schema.get_field("subtitle").unwrap();
        let title = schema.get_field("title").unwrap();

        let lang = schema.get_field("lang").unwrap();
        let license = schema.get_field("license").unwrap();

        let mut writer = self.writer.lock().unwrap();
        let writer = writer.as_mut().unwrap();
        writer.add_document(doc!(
            post_id => i64::from(post.id),
            author => post.get_authors(conn)?.into_iter().map(|u| u.fqn).join(" "),
            creation_date => i64::from(post.creation_date.num_days_from_ce()),
            instance => Instance::get(conn, post.get_blog(conn)?.instance_id)?.public_domain,
            tag => Tag::for_post(conn, post.id)?.into_iter().map(|t| t.tag).join(" "),
            blog_name => post.get_blog(conn)?.title,
            content => post.content.get().clone(),
            subtitle => post.subtitle.clone(),
            title => post.title.clone(),
            lang => detect_lang(post.content.get()).and_then(|i| if i.is_reliable() { Some(i.lang()) } else {None} ).unwrap_or(Lang::Eng).name(),
            license => post.license.clone(),
        ));
        Ok(())
    }

    pub fn delete_document(&self, post: &Post) {
        let schema = self.index.schema();
        let post_id = schema.get_field("post_id").unwrap();

        let doc_id = Term::from_field_i64(post_id, i64::from(post.id));
        let mut writer = self.writer.lock().unwrap();
        let writer = writer.as_mut().unwrap();
        writer.delete_term(doc_id);
    }

    pub fn update_document(&self, conn: &Connection, post: &Post) -> Result<()> {
        self.delete_document(post);
        self.add_document(conn, post)
    }

    pub fn search_document(
        &self,
        conn: &Connection,
        query: PlumeQuery,
        (min, max): (i32, i32),
    ) -> Vec<Post> {
        let schema = self.index.schema();
        let post_id = schema.get_field("post_id").unwrap();

        let collector = TopDocs::with_limit(cmp::max(1, max) as usize);

        let searcher = self.reader.searcher();
        let res = searcher.search(&query.into_query(), &collector).unwrap();

        res.get(min as usize..)
            .unwrap_or(&[])
            .iter()
            .filter_map(|(_, doc_add)| {
                let doc = searcher.doc(*doc_add).ok()?;
                let id = doc.get_first(post_id)?;
                Post::get(conn, id.i64_value() as i32).ok()
                //borrow checker don't want me to use filter_map or and_then here
            })
            .collect()
    }

    pub fn fill(&self, conn: &Connection) -> Result<()> {
        for post in posts::table
            .filter(posts::published.eq(true))
            .load::<Post>(conn)?
        {
            self.update_document(conn, &post)?
        }
        Ok(())
    }

    pub fn commit(&self) {
        let mut writer = self.writer.lock().unwrap();
        writer.as_mut().unwrap().commit().unwrap();
        self.reader.reload().unwrap();
    }

    pub fn drop_writer(&self) {
        self.writer.lock().unwrap().take();
    }
}
