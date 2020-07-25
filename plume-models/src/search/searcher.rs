use crate::{
    config::SearchTokenizerConfig, db_conn::DbPool, instance::Instance, posts::Post, schema::posts,
    search::query::PlumeQuery, tags::Tag, Error, Result, CONFIG,
};
use chrono::{Datelike, Utc};
use diesel::{ExpressionMethods, QueryDsl, RunQueryDsl};
use itertools::Itertools;
use rocket::request::{self, FromRequest, Outcome, Request, State};
use std::{cmp, fs::create_dir_all, io, path::Path, sync::Mutex};
use tantivy::{
    collector::TopDocs, directory::MmapDirectory, schema::*, Index, IndexReader, IndexWriter,
    ReloadPolicy, TantivyError, Term,
};
use whatlang::{detect as detect_lang, Lang};

#[derive(Debug)]
pub enum SearcherError {
    IndexCreationError,
    WriteLockAcquisitionError,
    IndexOpeningError,
    IndexEditionError,
    InvalidIndexDataError,
}

pub struct Searcher {
    index: Index,
    reader: IndexReader,
    writer: Mutex<Option<IndexWriter>>,
    dbpool: DbPool,
}

impl Searcher {
    /// Initializes a new `Searcher`, ready to be used by
    /// Plume.
    ///
    /// The main task of this function is to try everything
    /// to get a valid `Searcher`:
    ///
    /// - first it tries to open the search index normally (using the options from `CONFIG`)
    /// - if it fails, it makes a back-up of the index files, deletes the original ones,
    ///   and recreate the whole index. It removes the backup only if the re-creation
    ///   succeeds.
    ///
    /// # Panics
    ///
    /// This function panics if it needs to create a backup and it can't, or if it fails
    /// to recreate the search index.
    ///
    /// After that, it can also panic if there are still errors remaining.
    ///
    /// The panic messages are normally explicit enough for a human to
    /// understand how to fix the issue when they see it.
    pub fn new(db_pool: DbPool) -> Self {
        // We try to open the index a first time
        let searcher = match Self::open(
            &CONFIG.search_index,
            db_pool.clone(),
            &CONFIG.search_tokenizers,
        ) {
            // The index may be corrupted, inexistent or use an older format.
            // In this case, we can easily recover by deleting and re-creating it.
            Err(Error::Search(SearcherError::InvalidIndexDataError)) => {
                if Self::create(
                    &CONFIG.search_index,
                    db_pool.clone(),
                    &CONFIG.search_tokenizers,
                )
                .is_err()
                {
                    let current_path = Path::new(&CONFIG.search_index);
                    let backup_path =
                        format!("{}.{}", &current_path.display(), Utc::now().timestamp());
                    let backup_path = Path::new(&backup_path);
                    std::fs::rename(current_path, backup_path)
                        .expect("Error while backing up search index directory for re-creation");
                    if Self::create(
                        &CONFIG.search_index,
                        db_pool.clone(),
                        &CONFIG.search_tokenizers,
                    )
                    .is_ok()
                    {
                        if std::fs::remove_dir_all(backup_path).is_err() {
                            eprintln!(
                                "error on removing backup directory: {}. it remains",
                                backup_path.display()
                            );
                        }
                    } else {
                        panic!("Error while re-creating search index in new index format. Remove search index and run `plm search init` manually.");
                    }
                }
                Self::open(&CONFIG.search_index, db_pool, &CONFIG.search_tokenizers)
            }
            // If it opened successfully or if it was another kind of
            // error (that we don't know how to handle), don't do anything more
            other => other,
        };

        // At this point, if there are still errors, we just panic
        #[allow(clippy::match_wild_err_arm)]
        match searcher {
            Err(Error::Search(e)) => match e {
                SearcherError::WriteLockAcquisitionError => panic!(
                    r#"
Your search index is locked. Plume can't start. To fix this issue
make sure no other Plume instance is started, and run:

    plm search unlock

Then try to restart Plume.
"#
                ),
                SearcherError::IndexOpeningError => panic!(
                    r#"
Plume was unable to open the search index. If you created the index
before, make sure to run Plume in the same directory it was created in, or
to set SEARCH_INDEX accordingly. If you did not yet create the search
index, run this command:

    plm search init

Then try to restart Plume
"#
                ),
                e => Err(e).unwrap(),
            },
            Err(_) => panic!("Unexpected error while opening search index"),
            Ok(s) => s,
        }
    }

    pub fn schema() -> Schema {
        let tag_indexing = TextOptions::default().set_indexing_options(
            TextFieldIndexing::default()
                .set_tokenizer("tag_tokenizer")
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

    pub fn create(
        path: &dyn AsRef<Path>,
        dbpool: DbPool,
        tokenizers: &SearchTokenizerConfig,
    ) -> Result<Self> {
        let schema = Self::schema();

        create_dir_all(path).map_err(|_| SearcherError::IndexCreationError)?;
        let index = Index::create(
            MmapDirectory::open(path).map_err(|_| SearcherError::IndexCreationError)?,
            schema,
        )
        .map_err(|_| SearcherError::IndexCreationError)?;

        {
            let tokenizer_manager = index.tokenizers();
            tokenizer_manager.register("tag_tokenizer", tokenizers.tag_tokenizer);
            tokenizer_manager.register("content_tokenizer", tokenizers.content_tokenizer);
            tokenizer_manager.register("property_tokenizer", tokenizers.property_tokenizer);
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
            dbpool,
        })
    }

    pub fn open(
        path: &dyn AsRef<Path>,
        dbpool: DbPool,
        tokenizers: &SearchTokenizerConfig,
    ) -> Result<Self> {
        let mut index =
            Index::open(MmapDirectory::open(path).map_err(|_| SearcherError::IndexOpeningError)?)
                .map_err(|_| SearcherError::IndexOpeningError)?;

        {
            let tokenizer_manager = index.tokenizers();
            tokenizer_manager.register("tag_tokenizer", tokenizers.tag_tokenizer);
            tokenizer_manager.register("content_tokenizer", tokenizers.content_tokenizer);
            tokenizer_manager.register("property_tokenizer", tokenizers.property_tokenizer);
        } //to please the borrow checker
        let writer = index
            .writer(50_000_000)
            .map_err(|_| SearcherError::WriteLockAcquisitionError)?;

        // Since Tantivy v0.12.0, IndexWriter::garbage_collect_files() returns Future.
        // To avoid conflict with Plume async project, we don't introduce async now.
        // After async is introduced to Plume, we can use garbage_collect_files() again.
        // Algorithm stolen from Tantivy's SegmentUpdater::list_files()
        use std::collections::HashSet;
        use std::path::PathBuf;
        let mut files: HashSet<PathBuf> = index
            .list_all_segment_metas()
            .into_iter()
            .flat_map(|segment_meta| segment_meta.list_files())
            .collect();
        files.insert(Path::new("meta.json").to_path_buf());
        index
            .directory_mut()
            .garbage_collect(|| files)
            .map_err(|_| SearcherError::IndexEditionError)?;

        Ok(Self {
            writer: Mutex::new(Some(writer)),
            reader: index
                .reader_builder()
                .reload_policy(ReloadPolicy::Manual)
                .try_into()
                .map_err(|e| {
                    if let TantivyError::IOError(err) = e {
                        let err: io::Error = err.into();
                        if err.kind() == io::ErrorKind::InvalidData {
                            // Search index was created in older Tantivy format.
                            SearcherError::InvalidIndexDataError
                        } else {
                            SearcherError::IndexCreationError
                        }
                    } else {
                        SearcherError::IndexCreationError
                    }
                })?,
            index,
            dbpool,
        })
    }

    pub fn add_document(&self, post: &Post) -> Result<()> {
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

        let conn = match self.dbpool.get() {
            Ok(c) => c,
            Err(_) => return Err(Error::DbPool),
        };
        let mut writer = self.writer.lock().unwrap();
        let writer = writer.as_mut().unwrap();
        writer.add_document(doc!(
            post_id => i64::from(post.id),
            author => post.get_authors(&conn)?.into_iter().map(|u| u.fqn).join(" "),
            creation_date => i64::from(post.creation_date.num_days_from_ce()),
            instance => Instance::get(&conn, post.get_blog(&conn)?.instance_id)?.public_domain,
            tag => Tag::for_post(&conn, post.id)?.into_iter().map(|t| t.tag).join(" "),
            blog_name => post.get_blog(&conn)?.title,
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

    pub fn update_document(&self, post: &Post) -> Result<()> {
        self.delete_document(post);
        self.add_document(post)
    }

    pub fn search_document(&self, query: PlumeQuery, (min, max): (i32, i32)) -> Vec<Post> {
        let schema = self.index.schema();
        let post_id = schema.get_field("post_id").unwrap();

        let collector = TopDocs::with_limit(cmp::max(1, max) as usize);

        let searcher = self.reader.searcher();
        let res = searcher.search(&query.into_query(), &collector).unwrap();

        let conn = match self.dbpool.get() {
            Ok(c) => c,
            Err(_) => return Vec::new(),
        };

        res.get(min as usize..)
            .unwrap_or(&[])
            .iter()
            .filter_map(|(_, doc_add)| {
                let doc = searcher.doc(*doc_add).ok()?;
                let id = doc.get_first(post_id)?;
                Post::get(&conn, id.i64_value() as i32).ok()
                //borrow checker don't want me to use filter_map or and_then here
            })
            .collect()
    }

    pub fn fill(&self) -> Result<()> {
        let conn = match self.dbpool.get() {
            Ok(c) => c,
            Err(_) => return Err(Error::DbPool),
        };
        for post in posts::table
            .filter(posts::published.eq(true))
            .load::<Post>(&conn)?
        {
            self.update_document(&post)?
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

impl<'a, 'r> FromRequest<'a, 'r> for Searcher {
    type Error = ();

    fn from_request(request: &'a Request<'r>) -> request::Outcome<Searcher, Self::Error> {
        let searcher = request.guard::<State<'_, Searcher>>()?;
        Outcome::Success(*searcher.inner())
    }
}
