use crate::search::searcher::Searcher;
use chrono::{naive::NaiveDate, offset::Utc, Datelike};
use std::{cmp, ops::Bound};
use tantivy::{query::*, schema::*, Term};

//Generate functions for advanced search
macro_rules! gen_func {
    ( $($field:ident),*; strip: $($strip:ident),* ) => {
        $(  //most fields go here, it's kinda the "default" way
            pub fn $field(&mut self, mut val: &str, occur: Option<Occur>) -> &mut Self {
                if !val.trim_matches(&[' ', '"', '+', '-'][..]).is_empty() {
                    let occur = if let Some(occur) = occur {
                        occur
                    } else {
                        if val.get(0..1).map(|v| v=="+").unwrap_or(false) {
                            val = &val[1..];
                            Occur::Must
                        } else if val.get(0..1).map(|v| v=="-").unwrap_or(false) {
                            val = &val[1..];
                            Occur::MustNot
                        } else {
                            Occur::Should
                        }
                    };
                    self.$field.push((occur, val.trim_matches(&[' ', '"'][..]).to_owned()));
                }
                self
            }
        )*
        $(  // blog and author go here, leading @ get dismissed
            pub fn $strip(&mut self, mut val: &str, occur: Option<Occur>) -> &mut Self {
                if !val.trim_matches(&[' ', '"', '+', '-'][..]).is_empty() {
                    let occur = if let Some(occur) = occur {
                        occur
                    } else {
                        if val.get(0..1).map(|v| v=="+").unwrap_or(false) {
                            val = &val[1..];
                            Occur::Must
                        } else if val.get(0..1).map(|v| v=="-").unwrap_or(false) {
                            val = &val[1..];
                            Occur::MustNot
                        } else {
                            Occur::Should
                        }
                    };
                    self.$strip.push((occur, val.trim_matches(&[' ', '"', '@'][..]).to_owned()));
                }
                self
            }
        )*
    }
}

//generate the parser for advanced query from string
macro_rules! gen_parser {
    ( $self:ident, $query:ident, $occur:ident; normal: $($field:ident),*; date: $($date:ident),*) => {
        $(  // most fields go here
            if $query.starts_with(concat!(stringify!($field), ':')) {
                let new_query = &$query[concat!(stringify!($field), ':').len()..];
                let (token, rest) = Self::get_first_token(new_query);
                $query = rest;
                $self.$field(token, Some($occur));
            } else
        )*
        $(  // dates (before/after) got here
            if $query.starts_with(concat!(stringify!($date), ':')) {
                let new_query = &$query[concat!(stringify!($date), ':').len()..];
                let (token, rest) = Self::get_first_token(new_query);
                $query = rest;
                if let Ok(token) = NaiveDate::parse_from_str(token, "%Y-%m-%d") {
                    $self.$date(&token);
                }
            } else
        )*  // fields without 'fieldname:' prefix are considered bare words, and will be searched in title, subtitle and content
        {
            let (token, rest) = Self::get_first_token($query);
            $query = rest;
            $self.text(token, Some($occur));
        }
    }
}

// generate the to_string, giving back a textual query from a PlumeQuery
macro_rules! gen_to_string {
    ( $self:ident, $result:ident; normal: $($field:ident),*; date: $($date:ident),*) => {
        $(
        for (occur, val) in &$self.$field {
            if val.contains(' ') {
                $result.push_str(&format!("{}{}:\"{}\" ", Self::occur_to_str(*occur), stringify!($field), val));
            } else {
                $result.push_str(&format!("{}{}:{} ", Self::occur_to_str(*occur), stringify!($field), val));
            }
        }
        )*
        $(
        for val in &$self.$date {
            $result.push_str(&format!("{}:{} ", stringify!($date), NaiveDate::from_num_days_from_ce_opt(*val as i32).unwrap().format("%Y-%m-%d")));
        }
        )*
    }
}

// convert PlumeQuery to Tantivy's Query
macro_rules! gen_to_query {
    ( $self:ident, $result:ident; normal: $($normal:ident),*; oneoff: $($oneoff:ident),*) => {
        $(  // classic fields
            for (occur, token) in $self.$normal {
                $result.push((occur, Self::token_to_query(&token, stringify!($normal))));
            }
        )*
        $(  // fields where having more than on Must make no sense in general, so it's considered a Must be one of these instead.
            // Those fields are instance, author, blog, lang and license
            let mut subresult = Vec::new();
            for (occur, token) in $self.$oneoff {
                match occur {
                    Occur::Must => subresult.push((Occur::Should, Self::token_to_query(&token, stringify!($oneoff)))),
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
pub struct PlumeQuery {
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

impl PlumeQuery {
    /// Create a new empty Query
    pub fn new() -> Self {
        Default::default()
    }

    /// Parse a query string into this Query
    pub fn parse_query(&mut self, query: &str) -> &mut Self {
        self.from_str_req(query.trim())
    }

    /// Convert this Query to a Tantivy Query
    pub fn into_query(self) -> BooleanQuery {
        let mut result: Vec<(Occur, Box<dyn Query>)> = Vec::new();
        gen_to_query!(self, result; normal: title, subtitle, content, tag;
                      oneoff: instance, author, blog, lang, license);

        for (occur, token) in self.text {
            // text entries need to be added as multiple Terms
            match occur {
                Occur::Must => {
                    // a Must mean this must be in one of title subtitle or content, not in all 3
                    let subresult = vec![
                        (Occur::Should, Self::token_to_query(&token, "title")),
                        (Occur::Should, Self::token_to_query(&token, "subtitle")),
                        (Occur::Should, Self::token_to_query(&token, "content")),
                    ];

                    result.push((Occur::Must, Box::new(BooleanQuery::from(subresult))));
                }
                occur => {
                    result.push((occur, Self::token_to_query(&token, "title")));
                    result.push((occur, Self::token_to_query(&token, "subtitle")));
                    result.push((occur, Self::token_to_query(&token, "content")));
                }
            }
        }

        if self.before.is_some() || self.after.is_some() {
            // if at least one range bound is provided
            let after = self.after.unwrap_or_else(|| {
                i64::from(
                    NaiveDate::from_ymd_opt(2000, 1, 1)
                        .unwrap()
                        .num_days_from_ce(),
                )
            });
            let before = self
                .before
                .unwrap_or_else(|| i64::from(Utc::now().date_naive().num_days_from_ce()));
            let field = Searcher::schema().get_field("creation_date").unwrap();
            let range =
                RangeQuery::new_i64_bounds(field, Bound::Included(after), Bound::Included(before));
            result.push((Occur::Must, Box::new(range)));
        }

        result.into()
    }

    //generate most setters functions
    gen_func!(text, title, subtitle, content, tag, instance, lang, license; strip: author, blog);

    // documents newer than the provided date will be ignored
    pub fn before<D: Datelike>(&mut self, date: &D) -> &mut Self {
        let before = self
            .before
            .unwrap_or_else(|| i64::from(Utc::now().date_naive().num_days_from_ce()));
        self.before = Some(cmp::min(before, i64::from(date.num_days_from_ce())));
        self
    }

    // documents older than the provided date will be ignored
    pub fn after<D: Datelike>(&mut self, date: &D) -> &mut Self {
        let after = self.after.unwrap_or_else(|| {
            i64::from(
                NaiveDate::from_ymd_opt(2000, 1, 1)
                    .unwrap()
                    .num_days_from_ce(),
            )
        });
        self.after = Some(cmp::max(after, i64::from(date.num_days_from_ce())));
        self
    }

    // split a string into a token and a rest
    pub fn get_first_token(mut query: &str) -> (&str, &str) {
        query = query.trim();
        if query.is_empty() {
            ("", "")
        } else if query.get(0..1).map(|v| v == "\"").unwrap_or(false) {
            if let Some(index) = query[1..].find('"') {
                query.split_at(index + 2)
            } else {
                (query, "")
            }
        } else if query
            .get(0..2)
            .map(|v| v == "+\"" || v == "-\"")
            .unwrap_or(false)
        {
            if let Some(index) = query[2..].find('"') {
                query.split_at(index + 3)
            } else {
                (query, "")
            }
        } else if let Some(index) = query.find(' ') {
            query.split_at(index)
        } else {
            (query, "")
        }
    }

    // map each Occur state to a prefix
    fn occur_to_str(occur: Occur) -> &'static str {
        match occur {
            Occur::Should => "",
            Occur::Must => "+",
            Occur::MustNot => "-",
        }
    }

    // recursive parser for query string
    // allow this clippy lint for now, until someone figures out how to
    // refactor this better.
    #[allow(clippy::wrong_self_convention)]
    fn from_str_req(&mut self, mut query: &str) -> &mut Self {
        query = query.trim_start();
        if query.is_empty() {
            return self;
        }

        let occur = if query.get(0..1).map(|v| v == "+").unwrap_or(false) {
            query = &query[1..];
            Occur::Must
        } else if query.get(0..1).map(|v| v == "-").unwrap_or(false) {
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

    // map a token and it's field to a query
    fn token_to_query(token: &str, field_name: &str) -> Box<dyn Query> {
        let token = token.to_lowercase();
        let token = token.as_str();
        let field = Searcher::schema().get_field(field_name).unwrap();
        if token.contains('@') && (field_name == "author" || field_name == "blog") {
            let pos = token.find('@').unwrap();
            let user_term = Term::from_field_text(field, &token[..pos]);
            let instance_term = Term::from_field_text(
                Searcher::schema().get_field("instance").unwrap(),
                &token[pos + 1..],
            );
            Box::new(BooleanQuery::from(vec![
                (
                    Occur::Must,
                    Box::new(TermQuery::new(
                        user_term,
                        if field_name == "author" {
                            IndexRecordOption::Basic
                        } else {
                            IndexRecordOption::WithFreqsAndPositions
                        },
                    )) as Box<dyn Query + 'static>,
                ),
                (
                    Occur::Must,
                    Box::new(TermQuery::new(instance_term, IndexRecordOption::Basic)),
                ),
            ]))
        } else if token.contains(' ') {
            // phrase query
            match field_name {
                "instance" | "author" | "tag" =>
                // phrase query are not available on these fields, treat it as multiple Term queries
                {
                    Box::new(BooleanQuery::from(
                        token
                            .split_whitespace()
                            .map(|token| {
                                let term = Term::from_field_text(field, token);
                                (
                                    Occur::Should,
                                    Box::new(TermQuery::new(term, IndexRecordOption::Basic))
                                        as Box<dyn Query + 'static>,
                                )
                            })
                            .collect::<Vec<_>>(),
                    ))
                }
                _ => Box::new(PhraseQuery::new(
                    token
                        .split_whitespace()
                        .map(|token| Term::from_field_text(field, token))
                        .collect(),
                )),
            }
        } else {
            // Term Query
            let term = Term::from_field_text(field, token);
            let index_option = match field_name {
                "instance" | "author" | "tag" => IndexRecordOption::Basic,
                _ => IndexRecordOption::WithFreqsAndPositions,
            };
            Box::new(TermQuery::new(term, index_option))
        }
    }
}

impl std::str::FromStr for PlumeQuery {
    type Err = !;

    /// Create a new Query from &str
    /// Same as doing
    /// ```rust
    /// # extern crate plume_models;
    /// # use plume_models::search::Query;
    /// let mut q = Query::new();
    /// q.parse_query("some query");
    /// ```
    fn from_str(query: &str) -> Result<PlumeQuery, !> {
        let mut res: PlumeQuery = Default::default();

        res.from_str_req(query.trim());
        Ok(res)
    }
}

impl ToString for PlumeQuery {
    fn to_string(&self) -> String {
        let mut result = String::new();
        for (occur, val) in &self.text {
            if val.contains(' ') {
                result.push_str(&format!("{}\"{}\" ", Self::occur_to_str(*occur), val));
            } else {
                result.push_str(&format!("{}{} ", Self::occur_to_str(*occur), val));
            }
        }

        gen_to_string!(self, result; normal: title, subtitle, content, tag,
                      instance, author, blog, lang, license;
                      date: before, after);

        result.pop(); // remove trailing ' '
        result
    }
}
