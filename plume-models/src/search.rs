use instance::Instance;
use posts::Post;
use tags::Tag;
use Connection;

use chrono::{Datelike, naive::NaiveDate, offset::Utc};
use itertools::Itertools;
use tantivy::{
    collector::TopCollector,
    directory::MmapDirectory,
    query::{Query as TQuery, *},
    schema::*,
    tokenizer::*,
    Index, IndexWriter, Term
};
use whatlang::{detect as detect_lang, Lang};
use std::{
    cmp,
    fs::create_dir_all,
    ops::Bound,
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
    fn schema() -> Schema {
        let tag_indexing = TextOptions::default()
            .set_indexing_options(TextFieldIndexing::default()
                                  .set_tokenizer("whitespace_tokenizer")
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

        schema_builder.add_text_field("instance", tag_indexing.clone());
        schema_builder.add_text_field("author", tag_indexing.clone());//todo move to a user_indexing with user_tokenizer function
        schema_builder.add_text_field("tag", tag_indexing);

        schema_builder.add_text_field("blog", content_indexing.clone());
        schema_builder.add_text_field("content", content_indexing.clone());
        schema_builder.add_text_field("subtitle", content_indexing.clone());
        schema_builder.add_text_field("title", content_indexing);

        schema_builder.add_text_field("lang", property_indexing.clone());
        schema_builder.add_text_field("license", property_indexing);

        schema_builder.build()
    }


    pub fn create(path: &AsRef<Path>) -> Result<Self,SearcherError> {
        let whitespace_tokenizer = tokenizer::WhitespaceTokenizer
            .filter(LowerCaser);

        let content_tokenizer = SimpleTokenizer
            .filter(RemoveLongFilter::limit(40))
            .filter(LowerCaser);

        let property_tokenizer = NgramTokenizer::new(2, 8, false)
            .filter(LowerCaser);

        let schema = Self::schema();

        create_dir_all(path).map_err(|_| SearcherError::IndexCreationError)?;
        let index = Index::create(MmapDirectory::open(path).map_err(|_| SearcherError::IndexCreationError)?, schema).map_err(|_| SearcherError::IndexCreationError)?;

        {
            let tokenizer_manager = index.tokenizers();
            tokenizer_manager.register("whitespace_tokenizer", whitespace_tokenizer);
            tokenizer_manager.register("content_tokenizer", content_tokenizer);
            tokenizer_manager.register("property_tokenizer", property_tokenizer);
        }//to please the borrow checker
        Ok(Self {
            writer: Mutex::new(Some(index.writer(50_000_000).map_err(|_| SearcherError::WriteLockAcquisitionError)?)),
            index
        })
    }

    pub fn open(path: &AsRef<Path>) -> Result<Self, SearcherError> {
        let whitespace_tokenizer = tokenizer::WhitespaceTokenizer
            .filter(LowerCaser);

        let content_tokenizer = SimpleTokenizer
            .filter(RemoveLongFilter::limit(40))
            .filter(LowerCaser);

        let property_tokenizer = NgramTokenizer::new(2, 8, false)
            .filter(LowerCaser);

        let index = Index::open(MmapDirectory::open(path).map_err(|_| SearcherError::IndexOpeningError)?).map_err(|_| SearcherError::IndexOpeningError)?;

        {
            let tokenizer_manager = index.tokenizers();
            tokenizer_manager.register("whitespace_tokenizer", whitespace_tokenizer);
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
                author => post.get_authors(conn).into_iter().map(|u| u.get_fqn(conn)).join(" "),
                creation_date => i64::from(post.creation_date.num_days_from_ce()),
                instance => Instance::get(conn, post.get_blog(conn).instance_id).unwrap().public_domain.clone(),
                tag => Tag::for_post(conn, post.id).into_iter().map(|t| t.tag).join(" "),
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

    pub fn search_document(&self, conn: &Connection, query: Query, (min, max): (i32, i32)) -> Vec<Post>{
        let schema = self.index.schema();
        let post_id = schema.get_field("post_id").unwrap();

        let mut collector = TopCollector::with_limit(cmp::max(1,max) as usize);

        let searcher = self.index.searcher();
        searcher.search(&query.into_query(), &mut collector).unwrap();

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

//some macros to help with code duplication
macro_rules! gen_func {
    ( $($field:ident),*; decompose($inst:ident): $($dec:ident),* ) => {
        $(
            pub fn $field(&mut self, mut $field: &str, occur: Option<Occur>) -> &mut Self {
                if !$field.is_empty() {
                    let occur = if let Some(occur) = occur {
                        occur
                    } else {
                        if $field.get(0..1).map(|v| v=="+").unwrap_or(false) {
                            $field = &$field[1..];
                            Occur::Must
                        } else if $field.get(0..1).map(|v| v=="-").unwrap_or(false) {
                            $field = &$field[1..];
                            Occur::MustNot
                        } else {
                            Occur::Should
                        }
                    };
                    self.$field.push((occur, $field.to_owned()));
                }
                self
            }
        )*
        $(
            pub fn $dec(&mut self, mut $dec: &str, occur: Option<Occur>) -> &mut Self {

                if !$dec.is_empty() {
                    let occur = if let Some(occur) = occur {
                        occur
                    } else {
                        if $dec.get(0..1).map(|v| v=="+").unwrap_or(false) {
                            $dec = &$dec[1..];
                            Occur::Must
                        } else if $dec.get(0..1).map(|v| v=="-").unwrap_or(false) {
                            $dec = &$dec[1..];
                            Occur::MustNot
                        } else {
                            Occur::Should
                        }
                    };
                    $dec = $dec.trim_left_matches('@');
                    if let Some(pos) = $dec.find('@') {
                        let (name, domain) = $dec.split_at(pos);
                        self.$dec.push((occur, name.to_owned()));
                        self.$inst.push((occur, domain[1..].to_owned()));

                    } else {
                        self.$dec.push((occur, $dec.to_owned()));
                    }
                }
                self
            }
        )*
    }
}

macro_rules! gen_parser {
    ( $self:ident, $query:ident, $occur:ident; normal: $($field:ident),*; date: $($date:ident),*) => {
        if false {
            unreachable!();
        }
        $(
            else if $query.starts_with(concat!(stringify!($field), ':')) {
                let new_query = &$query[concat!(stringify!($field), ':').len()..];
                let (token, rest) = Self::get_first_token(new_query);
                $query = rest;
                $self.$field(token, Some($occur));
            }
        )*
        $(
            else if $query.starts_with(concat!(stringify!($date), ':')) {
                let new_query = &$query[concat!(stringify!($date), ':').len()..];
                let (token, rest) = Self::get_first_token(new_query);
                $query = rest;
                if let Ok(token) = NaiveDate::parse_from_str(token, "%Y-%m-%d") {
                    $self.$date(&token);
                }
            }
        )*
        else {
            let (token, rest) = Self::get_first_token($query);
            $query = rest;
            $self.text(token, Some($occur));
        }
    }
}

macro_rules! gen_to_string {
    ( $self:ident, $result:ident; normal: $($field:ident),*; date: $($date:ident),*) => {
        $(
        for (occur, val) in &$self.$field {
            if val.contains(' ') {
                $result.push_str(&format!("{}{}:\"{}\" ", Self::occur_to_str(&occur), stringify!($field), val));
            } else {
                $result.push_str(&format!("{}{}:{} ", Self::occur_to_str(&occur), stringify!($field), val));
            }
        }
        )*
        $(
        for val in &$self.$date {
            $result.push_str(&format!("{}:{} ", stringify!($date), NaiveDate::from_num_days_from_ce(*val as i32).format("%Y-%m-%d")));
        }
        )*
    }
}

macro_rules! gen_to_query {
    ( $self:ident, $result:ident; normal: $($normal:ident),*; oneoff: $($oneoff:ident),*) => {
        $(
            for (occur, token) in $self.$normal {
                $result.push((occur, Self::token_to_query(&token, stringify!($normal))));
            }
        )*
        $(
            let mut subresult = Vec::new();
            for (occur, token) in $self.$oneoff {
                match occur {
                    Occur::Should => subresult.push((Occur::Should, Self::token_to_query(&token, stringify!($oneoff)))),
                    occur => $result.push((occur, Self::token_to_query(&token, stringify!($oneoff)))),
                }
            }
            if !subresult.is_empty() {
                $result.push((Occur::Must, Box::new(BooleanQuery::from(subresult))));
            }
        )*
    }
}

#[derive(Default)]
pub struct Query {
    text: Vec<(Occur, String)>,
    title: Vec<(Occur, String)>,
    subtitle: Vec<(Occur, String)>,
    content: Vec<(Occur, String)>,
    tag: Vec<(Occur, String)>,
    instance: Vec<(Occur, String)>,
    author: Vec<(Occur, String)>,
    blog: Vec<(Occur, String)>,
    lang: Vec<(Occur, String)>,
    license: Vec<(Occur, String)>,
    before: Option<i64>,
    after: Option<i64>,
}

impl Query {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn from_str(query: &str) -> Self {
        let mut res: Self = Default::default();

        res.from_str_req(&query.trim());
        res
    }

    pub fn parse_query(&mut self, query: &str) -> &mut Self {
        self.from_str_req(&query.trim())
    }

    pub fn into_query(self) -> BooleanQuery {
        let mut result = Vec::new();
        gen_to_query!(self, result; normal: title, subtitle, content, tag;
                      oneoff: instance, author, blog, lang, license);

        for (occur, token) in self.text {
            match occur {
                Occur::Must => {
                    let subresult = vec![
                        (Occur::Should, Self::token_to_query(&token, "title")),
                        (Occur::Should, Self::token_to_query(&token, "title")),
                        (Occur::Should, Self::token_to_query(&token, "title")),
                    ];

                    result.push((Occur::Must, Box::new(BooleanQuery::from(subresult))));
                },
                occur => {
                    result.push((occur, Self::token_to_query(&token, "title")));
                    result.push((occur, Self::token_to_query(&token, "subtitle")));
                    result.push((occur, Self::token_to_query(&token, "content")));
                },
            }
        }

        if self.before.is_some() || self.after.is_some() {
            let after = self.after.unwrap_or_else(|| i64::from(NaiveDate::from_ymd(2000, 1, 1).num_days_from_ce()));
            let before = self.before.unwrap_or_else(|| i64::from(Utc::today().num_days_from_ce()));
            let field = Searcher::schema().get_field("creation_date").unwrap();
            let range = RangeQuery::new_i64_bounds(field, Bound::Included(after), Bound::Included(before));
            result.push((Occur::Must, Box::new(range)));
        }

        result.into()
    }

    gen_func!(text, title, subtitle, content, tag, instance, lang, license; decompose(instance): author, blog);

    pub fn before<D: Datelike>(&mut self, date: &D) -> &mut Self {
        let before = self.before.unwrap_or_else(|| i64::from(Utc::today().num_days_from_ce()));
        self.before = Some(cmp::min(before, i64::from(date.num_days_from_ce())));
        self
    }

    pub fn after<D: Datelike>(&mut self, date: &D) -> &mut Self {
        let after = self.after.unwrap_or_else(|| i64::from(NaiveDate::from_ymd(2000, 1, 1).num_days_from_ce()));
        self.after = Some(cmp::max(after, i64::from(date.num_days_from_ce())));
        self
    }

    pub fn get_first_token<'a>(query: &'a str) -> (&'a str, &'a str) {
        if query.is_empty() {
            (query, query)
        } else {
            if query.get(0..1).map(|v| v=="\"").unwrap_or(false) {
                let mut iter = query[1..].splitn(2, '"');
                (iter.next().unwrap_or(&""), iter.next().unwrap_or(&""))
            } else {
                let mut iter = query.splitn(2, ' ');
                (iter.next().unwrap_or(&""), iter.next().unwrap_or(&""))
            }
        }
    }

    fn occur_to_str(occur: &Occur) -> &'static str {
        match occur {
            Occur::Should => "",
            Occur::Must => "+",
            Occur::MustNot => "-",
        }
    }

    fn from_str_req(&mut self, mut query: &str) -> &mut Self {
        if query.is_empty() {
            self
        } else {
            let occur = if query.get(0..1).map(|v| v=="+").unwrap_or(false) {
                query = &query[1..];
                Occur::Must
            } else if query.get(0..1).map(|v| v=="-").unwrap_or(false) {
                query = &query[1..];
                Occur::MustNot
            } else {
                Occur::Should
            };
            gen_parser!(self, query, occur; normal: title, subtitle, content, tag,
                            instance, author, blog, lang, license;
                            date: after, before);
            self.from_str_req(query)
        }
    }

    fn token_to_query(token: &str, field_name: &str) -> Box<TQuery> {
        let token = token.to_lowercase();
        let token = token.as_str();
        let field = Searcher::schema().get_field(field_name).unwrap();
        if token.contains(' ') {
            match field_name {
                "instance" | "author" | "tag" =>
                    Box::new(BooleanQuery::from(token.split_whitespace()
                                               .map(|token| {
                                                   let term = Term::from_field_text(field, token);
                                                   (Occur::Should, Box::new(TermQuery::new(term, IndexRecordOption::Basic))
                                                                   as Box<dyn TQuery + 'static>)
                                               })
                                               .collect::<Vec<_>>())),
                _ => Box::new(PhraseQuery::new(token.split_whitespace()
                                               .map(|token| Term::from_field_text(field, token))
                                               .collect()))
            }
        } else {
            let term = Term::from_field_text(field, token);
            let index_option = match field_name {
                "instance" | "author" | "tag" => IndexRecordOption::Basic,
                _ => IndexRecordOption::WithFreqsAndPositions,
            };
            Box::new(TermQuery::new(term, index_option))
        }
    }
}


impl ToString for Query {
    fn to_string(&self) -> String {
        let mut result = String::new();
        for (occur, val) in &self.text {
            result.push_str(&format!("{}{} ", Self::occur_to_str(&occur), val));
        }

        gen_to_string!(self, result; normal: title, subtitle, content, tag,
                      instance, author, blog, lang, license;
                      date: before, after);

        result.trim().to_owned()
    }
}

mod tokenizer {
    use std::str::CharIndices;
    use tantivy::tokenizer::{Token, TokenStream, Tokenizer};

    /// Tokenize the text by splitting on whitespaces.
    #[derive(Clone)]
    pub struct WhitespaceTokenizer;

    pub struct WhitespaceTokenStream<'a> {
        text: &'a str,
        chars: CharIndices<'a>,
        token: Token,
    }

    impl<'a> Tokenizer<'a> for WhitespaceTokenizer {
        type TokenStreamImpl = WhitespaceTokenStream<'a>;

        fn token_stream(&self, text: &'a str) -> Self::TokenStreamImpl {
            WhitespaceTokenStream {
                text,
                chars: text.char_indices(),
                token: Token::default(),
            }
        }
    }

    impl<'a> WhitespaceTokenStream<'a> {
        // search for the end of the current token.
        fn search_token_end(&mut self) -> usize {
            (&mut self.chars)
                .filter(|&(_, ref c)| c.is_whitespace())
                .map(|(offset, _)| offset)
                .next()
                .unwrap_or_else(|| self.text.len())
        }
    }

    impl<'a> TokenStream for WhitespaceTokenStream<'a> {
        fn advance(&mut self) -> bool {
            self.token.text.clear();
            self.token.position = self.token.position.wrapping_add(1);

            loop {
                match self.chars.next() {
                    Some((offset_from, c)) => {
                        if !c.is_whitespace() {
                            let offset_to = self.search_token_end();
                            self.token.offset_from = offset_from;
                            self.token.offset_to = offset_to;
                            self.token.text.push_str(&self.text[offset_from..offset_to]);
                            return true;
                        }
                    }
                    None => {
                        return false;
                    }
                }
            }
        }

        fn token(&self) -> &Token {
            &self.token
        }

        fn token_mut(&mut self) -> &mut Token {
            &mut self.token
        }
    }
}

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
