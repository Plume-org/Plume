use crate::{
    blogs::Blog,
    lists::{self, ListType},
    posts::Post,
    tags::Tag,
    timeline::Timeline,
    users::User,
    Connection, Result,
};
use plume_common::activity_pub::inbox::AsActor;
use whatlang::{self, Lang};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QueryError {
    SyntaxError(usize, usize, String),
    UnexpectedEndOfQuery,
    RuntimeError(String),
}

pub type QueryResult<T> = std::result::Result<T, QueryError>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Kind {
    Original,
    Reshare(User),
    Like(User),
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Token<'a> {
    LParent(usize),
    RParent(usize),
    LBracket(usize),
    RBracket(usize),
    Comma(usize),
    Word(usize, usize, &'a str),
}

impl<'a> Token<'a> {
    fn get_text(&self) -> &'a str {
        match self {
            Token::Word(_, _, s) => s,
            Token::LParent(_) => "(",
            Token::RParent(_) => ")",
            Token::LBracket(_) => "[",
            Token::RBracket(_) => "]",
            Token::Comma(_) => ",",
        }
    }

    fn get_pos(&self) -> (usize, usize) {
        match self {
            Token::Word(a, b, _) => (*a, *b),
            Token::LParent(a)
            | Token::RParent(a)
            | Token::LBracket(a)
            | Token::RBracket(a)
            | Token::Comma(a) => (*a, 1),
        }
    }

    fn get_error<T>(&self, token: Token<'_>) -> QueryResult<T> {
        let (b, e) = self.get_pos();
        let message = format!(
            "Syntax Error: Expected {}, got {}",
            token.to_string(),
            self.to_string()
        );
        Err(QueryError::SyntaxError(b, e, message))
    }
}

impl<'a> ToString for Token<'a> {
    fn to_string(&self) -> String {
        if let Token::Word(0, 0, v) = self {
            return (*v).to_string();
        }
        format!(
            "'{}'",
            match self {
                Token::Word(_, _, v) => v,
                Token::LParent(_) => "(",
                Token::RParent(_) => ")",
                Token::LBracket(_) => "[",
                Token::RBracket(_) => "]",
                Token::Comma(_) => ",",
            }
        )
    }
}

macro_rules! gen_tokenizer {
    ( ($c:ident,$i:ident), $state:ident, $quote:ident; $([$char:tt, $variant:tt]),*) => {
        match $c {
            space if !*$quote && space.is_whitespace() => match $state.take() {
                Some(v) => vec![v],
                None => vec![],
            },
            $(
                $char if !*$quote => match $state.take() {
                    Some(v) => vec![v, Token::$variant($i)],
                    None => vec![Token::$variant($i)],
                },
            )*
            '"' => {
                *$quote = !*$quote;
                vec![]
            },
            _ => match $state.take() {
                Some(Token::Word(b, l, _)) => {
                    *$state = Some(Token::Word(b, l+1, &""));
                    vec![]
                },
                None => {
                    *$state = Some(Token::Word($i,1,&""));
                    vec![]
                },
                _ => unreachable!(),
            }
        }
    }
}

fn lex(stream: &str) -> Vec<Token<'_>> {
    stream
        .chars()
        .chain(" ".chars()) // force a last whitespace to empty scan's state
        .zip(0..)
        .scan((None, false), |(state, quote), (c, i)| {
            Some(gen_tokenizer!((c,i), state, quote;
                                ['(', LParent],  [')', RParent],
                                ['[', LBracket], [']', RBracket],
                                [',', Comma]))
        })
        .flatten()
        .map(|t| {
            if let Token::Word(b, e, _) = t {
                Token::Word(b, e, &stream[b..b + e])
            } else {
                t
            }
        })
        .collect()
}

/// Private internals of TimelineQuery
#[derive(Debug, Clone, PartialEq)]
enum TQ<'a> {
    Or(Vec<TQ<'a>>),
    And(Vec<TQ<'a>>),
    Arg(Arg<'a>, bool),
}

impl<'a> TQ<'a> {

    #[async_recursion::async_recursion]
    async fn matches(
        &self,
        conn: &mut Connection,
        timeline: &Timeline,
        post: &Post,
        kind: &Kind,
    ) -> Result<bool> {
        match self {
            TQ::Or(inner) =>  {
                let mut res = false;
                for e in inner {
                    res = e.matches(conn, timeline, post, kind).await?;
                    if res {
                        break;
                    }
                }
                Ok(res)
            },
            TQ::And(inner) => {
                let mut res = true;
                for e in inner {
                    res = res && e.matches(conn, timeline, post, kind).await?;
                    if !res {
                        break;
                    }
                }
                Ok(res)
            },
            TQ::Arg(inner, invert) => Ok(inner.matches(conn, timeline, post, kind).await? ^ invert),
        }
    }

    fn list_used_lists(&self) -> Vec<(String, ListType)> {
        match self {
            TQ::Or(inner) => inner.iter().flat_map(TQ::list_used_lists).collect(),
            TQ::And(inner) => inner.iter().flat_map(TQ::list_used_lists).collect(),
            TQ::Arg(Arg::In(typ, List::List(name)), _) => vec![(
                (*name).to_string(),
                match typ {
                    WithList::Blog => ListType::Blog,
                    WithList::Author { .. } => ListType::User,
                    WithList::License => ListType::Word,
                    WithList::Tags => ListType::Word,
                    WithList::Lang => ListType::Prefix,
                },
            )],
            TQ::Arg(_, _) => vec![],
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
enum Arg<'a> {
    In(WithList, List<'a>),
    Contains(WithContains, &'a str),
    Boolean(Bool),
}

impl<'a> Arg<'a> {
    pub async fn matches(
        &self,
        conn: &mut Connection,
        timeline: &Timeline,
        post: &Post,
        kind: &Kind,
    ) -> Result<bool> {
        match self {
            Arg::In(t, l) => t.matches(conn, timeline, post, l, kind).await,
            Arg::Contains(t, v) => t.matches(post, v),
            Arg::Boolean(t) => t.matches(conn, timeline, post, kind),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
enum WithList {
    Blog,
    Author { boosts: bool, likes: bool },
    License,
    Tags,
    Lang,
}

impl WithList {
    pub async fn matches(
        &self,
        conn: &mut Connection,
        timeline: &Timeline,
        post: &Post,
        list: &List<'_>,
        kind: &Kind,
    ) -> Result<bool> {
        match list {
            List::List(name) => {
                let list = lists::List::find_for_user_by_name(conn, timeline.user_id, name)?;
                match (self, list.kind()) {
                    (WithList::Blog, ListType::Blog) => list.contains_blog(conn, post.blog_id),
                    (WithList::Author { boosts, likes }, ListType::User) => match kind {
                        Kind::Original => Ok(list
                            .list_users(conn)?
                            .iter()
                            .any(|a| post.is_author(conn, a.id).unwrap_or(false))),
                        Kind::Reshare(u) => {
                            if *boosts {
                                list.contains_user(conn, u.id)
                            } else {
                                Ok(false)
                            }
                        }
                        Kind::Like(u) => {
                            if *likes {
                                list.contains_user(conn, u.id)
                            } else {
                                Ok(false)
                            }
                        }
                    },
                    (WithList::License, ListType::Word) => list.contains_word(conn, &post.license),
                    (WithList::Tags, ListType::Word) => {
                        let tags = Tag::for_post(conn, post.id)?;
                        Ok(list
                            .list_words(conn)?
                            .iter()
                            .any(|s| tags.iter().any(|t| s == &t.tag)))
                    }
                    (WithList::Lang, ListType::Prefix) => {
                        let lang = whatlang::detect(post.content.get())
                            .and_then(|i| {
                                if i.is_reliable() {
                                    Some(i.lang())
                                } else {
                                    None
                                }
                            })
                            .unwrap_or(Lang::Eng)
                            .name();
                        list.contains_prefix(conn, lang)
                    }
                    (_, _) => Err(QueryError::RuntimeError(format!(
                        "The list '{}' is of the wrong type for this usage",
                        name
                    ))
                    .into()),
                }
            }
            List::Array(list) => match self {
                WithList::Blog => {
                    let mut res = false;
                    for l in list {
                        if let Ok(b) = Blog::find_by_fqn(conn, l).await {
                            if b.id == post.blog_id {
                                res = true;
                                break;
                            }
                        };
                    }
                    Ok(res)
                },
                WithList::Author { boosts, likes } => match kind {
                    Kind::Original => {
                        let mut res = false;
                        for l in list {
                            if let Ok(u) = User::find_by_fqn(conn, l).await {
                                if post.is_author(conn, u.id).unwrap_or(false) {
                                    res = true;
                                    break;
                                }
                            };
                        }
                        Ok(res)
                    },
                    Kind::Reshare(u) => {
                        if *boosts {
                            Ok(list.iter().any(|user| &u.fqn == user))
                        } else {
                            Ok(false)
                        }
                    }
                    Kind::Like(u) => {
                        if *likes {
                            Ok(list.iter().any(|user| &u.fqn == user))
                        } else {
                            Ok(false)
                        }
                    }
                },
                WithList::License => Ok(list.iter().any(|s| s == &post.license)),
                WithList::Tags => {
                    let tags = Tag::for_post(conn, post.id)?;
                    Ok(list.iter().any(|s| tags.iter().any(|t| s == &t.tag)))
                }
                WithList::Lang => {
                    let lang = whatlang::detect(post.content.get())
                        .and_then(|i| {
                            if i.is_reliable() {
                                Some(i.lang())
                            } else {
                                None
                            }
                        })
                        .unwrap_or(Lang::Eng)
                        .name()
                        .to_lowercase();
                    Ok(list.iter().any(|s| lang.starts_with(&s.to_lowercase())))
                }
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
enum WithContains {
    Title,
    Subtitle,
    Content,
}

impl WithContains {
    pub fn matches(&self, post: &Post, value: &str) -> Result<bool> {
        match self {
            WithContains::Title => Ok(post.title.contains(value)),
            WithContains::Subtitle => Ok(post.subtitle.contains(value)),
            WithContains::Content => Ok(post.content.contains(value)),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
enum Bool {
    Followed { boosts: bool, likes: bool },
    HasCover,
    Local,
    All,
}

impl Bool {
    pub fn matches(
        &self,
        conn: &mut Connection,
        timeline: &Timeline,
        post: &Post,
        kind: &Kind,
    ) -> Result<bool> {
        match self {
            Bool::Followed { boosts, likes } => {
                if timeline.user_id.is_none() {
                    return Ok(false);
                }
                let user = timeline.user_id.unwrap();
                match kind {
                    Kind::Original => post
                        .get_authors(conn)?
                        .iter()
                        .try_fold(false, |s, a| a.is_followed_by(conn, user).map(|r| s || r)),
                    Kind::Reshare(u) => {
                        if *boosts {
                            u.is_followed_by(conn, user)
                        } else {
                            Ok(false)
                        }
                    }
                    Kind::Like(u) => {
                        if *likes {
                            u.is_followed_by(conn, user)
                        } else {
                            Ok(false)
                        }
                    }
                }
            }
            Bool::HasCover => Ok(post.cover_id.is_some()),
            Bool::Local => Ok(post.get_blog(conn)?.is_local() && *kind == Kind::Original),
            Bool::All => Ok(*kind == Kind::Original),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
enum List<'a> {
    List(&'a str),
    Array(Vec<&'a str>),
}

fn parse_s<'a, 'b>(mut stream: &'b [Token<'a>]) -> QueryResult<(&'b [Token<'a>], TQ<'a>)> {
    let mut res = Vec::new();
    let (left, token) = parse_a(stream)?;
    res.push(token);
    stream = left;
    while !stream.is_empty() {
        match stream[0] {
            Token::Word(_, _, and) if and == "or" => {}
            _ => break,
        }
        let (left, token) = parse_a(&stream[1..])?;
        res.push(token);
        stream = left;
    }

    if res.len() == 1 {
        Ok((stream, res.remove(0)))
    } else {
        Ok((stream, TQ::Or(res)))
    }
}

fn parse_a<'a, 'b>(mut stream: &'b [Token<'a>]) -> QueryResult<(&'b [Token<'a>], TQ<'a>)> {
    let mut res = Vec::new();
    let (left, token) = parse_b(stream)?;
    res.push(token);
    stream = left;
    while !stream.is_empty() {
        match stream[0] {
            Token::Word(_, _, and) if and == "and" => {}
            _ => break,
        }
        let (left, token) = parse_b(&stream[1..])?;
        res.push(token);
        stream = left;
    }

    if res.len() == 1 {
        Ok((stream, res.remove(0)))
    } else {
        Ok((stream, TQ::And(res)))
    }
}

fn parse_b<'a, 'b>(stream: &'b [Token<'a>]) -> QueryResult<(&'b [Token<'a>], TQ<'a>)> {
    match stream.get(0) {
        Some(Token::LParent(_)) => {
            let (left, token) = parse_s(&stream[1..])?;
            match left.get(0) {
                Some(Token::RParent(_)) => Ok((&left[1..], token)),
                Some(t) => t.get_error(Token::RParent(0)),
                None => Err(QueryError::UnexpectedEndOfQuery),
            }
        }
        _ => parse_c(stream),
    }
}

fn parse_c<'a, 'b>(stream: &'b [Token<'a>]) -> QueryResult<(&'b [Token<'a>], TQ<'a>)> {
    match stream.get(0) {
        Some(Token::Word(_, _, not)) if not == &"not" => {
            let (left, token) = parse_d(&stream[1..])?;
            Ok((left, TQ::Arg(token, true)))
        }
        _ => {
            let (left, token) = parse_d(stream)?;
            Ok((left, TQ::Arg(token, false)))
        }
    }
}

fn parse_d<'a, 'b>(mut stream: &'b [Token<'a>]) -> QueryResult<(&'b [Token<'a>], Arg<'a>)> {
    match stream
        .get(0)
        .map(Token::get_text)
        .ok_or(QueryError::UnexpectedEndOfQuery)?
    {
        s @ "blog" | s @ "author" | s @ "license" | s @ "tags" | s @ "lang" => {
            match stream.get(1).ok_or(QueryError::UnexpectedEndOfQuery)? {
                Token::Word(_, _, r#in) if r#in == &"in" => {
                    let (mut left, list) = parse_l(&stream[2..])?;
                    let kind = match s {
                        "blog" => WithList::Blog,
                        "author" => {
                            let mut boosts = true;
                            let mut likes = false;
                            while let Some(Token::Word(s, e, clude)) = left.get(0) {
                                if *clude != "include" && *clude != "exclude" {
                                    break;
                                }
                                match (
                                    *clude,
                                    left.get(1)
                                        .map(Token::get_text)
                                        .ok_or(QueryError::UnexpectedEndOfQuery)?,
                                ) {
                                    ("include", "reshares") | ("include", "reshare") => {
                                        boosts = true
                                    }
                                    ("exclude", "reshares") | ("exclude", "reshare") => {
                                        boosts = false
                                    }
                                    ("include", "likes") | ("include", "like") => likes = true,
                                    ("exclude", "likes") | ("exclude", "like") => likes = false,
                                    (_, w) => {
                                        return Token::Word(*s, *e, w).get_error(Token::Word(
                                            0,
                                            0,
                                            "one of 'likes' or 'reshares'",
                                        ))
                                    }
                                }
                                left = &left[2..];
                            }
                            WithList::Author { boosts, likes }
                        }
                        "license" => WithList::License,
                        "tags" => WithList::Tags,
                        "lang" => WithList::Lang,
                        _ => unreachable!(),
                    };
                    Ok((left, Arg::In(kind, list)))
                }
                t => t.get_error(Token::Word(0, 0, "'in'")),
            }
        }
        s @ "title" | s @ "subtitle" | s @ "content" => match (
            stream.get(1).ok_or(QueryError::UnexpectedEndOfQuery)?,
            stream.get(2).ok_or(QueryError::UnexpectedEndOfQuery)?,
        ) {
            (Token::Word(_, _, contains), Token::Word(_, _, w)) if contains == &"contains" => Ok((
                &stream[3..],
                Arg::Contains(
                    match s {
                        "title" => WithContains::Title,
                        "subtitle" => WithContains::Subtitle,
                        "content" => WithContains::Content,
                        _ => unreachable!(),
                    },
                    w,
                ),
            )),
            (Token::Word(_, _, contains), t) if contains == &"contains" => {
                t.get_error(Token::Word(0, 0, "any word"))
            }
            (t, _) => t.get_error(Token::Word(0, 0, "'contains'")),
        },
        s @ "followed" | s @ "has_cover" | s @ "local" | s @ "all" => match s {
            "followed" => {
                let mut boosts = true;
                let mut likes = false;
                while let Some(Token::Word(s, e, clude)) = stream.get(1) {
                    if *clude != "include" && *clude != "exclude" {
                        break;
                    }
                    match (
                        *clude,
                        stream
                            .get(2)
                            .map(Token::get_text)
                            .ok_or(QueryError::UnexpectedEndOfQuery)?,
                    ) {
                        ("include", "reshares") | ("include", "reshare") => boosts = true,
                        ("exclude", "reshares") | ("exclude", "reshare") => boosts = false,
                        ("include", "likes") | ("include", "like") => likes = true,
                        ("exclude", "likes") | ("exclude", "like") => likes = false,
                        (_, w) => {
                            return Token::Word(*s, *e, w).get_error(Token::Word(
                                0,
                                0,
                                "one of 'likes' or 'boosts'",
                            ))
                        }
                    }
                    stream = &stream[2..];
                }
                Ok((&stream[1..], Arg::Boolean(Bool::Followed { boosts, likes })))
            }
            "has_cover" => Ok((&stream[1..], Arg::Boolean(Bool::HasCover))),
            "local" => Ok((&stream[1..], Arg::Boolean(Bool::Local))),
            "all" => Ok((&stream[1..], Arg::Boolean(Bool::All))),
            _ => unreachable!(),
        },
        _ => stream
            .get(0)
            .ok_or(QueryError::UnexpectedEndOfQuery)?
            .get_error(Token::Word(
                0,
                0,
                "one of 'blog', 'author', 'license', 'tags', 'lang', \
             'title', 'subtitle', 'content', 'followed', 'has_cover', 'local' or 'all'",
            )),
    }
}

fn parse_l<'a, 'b>(stream: &'b [Token<'a>]) -> QueryResult<(&'b [Token<'a>], List<'a>)> {
    match stream.get(0).ok_or(QueryError::UnexpectedEndOfQuery)? {
        Token::LBracket(_) => {
            let (left, list) = parse_m(&stream[1..])?;
            match left.get(0).ok_or(QueryError::UnexpectedEndOfQuery)? {
                Token::RBracket(_) => Ok((&left[1..], List::Array(list))),
                t => t.get_error(Token::Word(0, 0, "one of ']' or ','")),
            }
        }
        Token::Word(_, _, list) => Ok((&stream[1..], List::List(list))),
        t => t.get_error(Token::Word(0, 0, "one of [list, of, words] or list_name")),
    }
}

fn parse_m<'a, 'b>(mut stream: &'b [Token<'a>]) -> QueryResult<(&'b [Token<'a>], Vec<&'a str>)> {
    let mut res: Vec<&str> = vec![
        match stream.get(0).ok_or(QueryError::UnexpectedEndOfQuery)? {
            Token::Word(_, _, w) => w,
            t => return t.get_error(Token::Word(0, 0, "any word")),
        },
    ];
    stream = &stream[1..];
    while let Token::Comma(_) = stream[0] {
        res.push(
            match stream.get(1).ok_or(QueryError::UnexpectedEndOfQuery)? {
                Token::Word(_, _, w) => w,
                t => return t.get_error(Token::Word(0, 0, "any word")),
            },
        );
        stream = &stream[2..];
    }

    Ok((stream, res))
}

#[derive(Debug, Clone)]
pub struct TimelineQuery<'a>(TQ<'a>);

impl<'a> TimelineQuery<'a> {
    pub fn parse(query: &'a str) -> QueryResult<Self> {
        parse_s(&lex(query))
            .and_then(|(left, res)| {
                if left.is_empty() {
                    Ok(res)
                } else {
                    left[0].get_error(Token::Word(0, 0, "on of 'or' or 'and'"))
                }
            })
            .map(TimelineQuery)
    }

    pub async fn matches(
        &self,
        conn: &mut Connection,
        timeline: &Timeline,
        post: &Post,
        kind: &Kind,
    ) -> Result<bool> {
        self.0.matches(conn, timeline, post, kind).await
    }

    pub fn list_used_lists(&self) -> Vec<(String, ListType)> {
        self.0.list_used_lists()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lexer() {
        assert_eq!(
            lex("()[ ],two words \"something quoted with , and [\""),
            vec![
                Token::LParent(0),
                Token::RParent(1),
                Token::LBracket(2),
                Token::RBracket(4),
                Token::Comma(5),
                Token::Word(6, 3, "two"),
                Token::Word(10, 5, "words"),
                Token::Word(17, 29, "something quoted with , and ["),
            ]
        );
    }

    #[test]
    fn test_parser() {
        let q = TimelineQuery::parse(r#"lang in [fr, en] and (license in my_fav_lic or not followed) or title contains "Plume is amazing""#)
            .unwrap();
        assert_eq!(
            q.0,
            TQ::Or(vec![
                TQ::And(vec![
                    TQ::Arg(
                        Arg::In(WithList::Lang, List::Array(vec!["fr", "en"]),),
                        false
                    ),
                    TQ::Or(vec![
                        TQ::Arg(Arg::In(WithList::License, List::List("my_fav_lic"),), false),
                        TQ::Arg(
                            Arg::Boolean(Bool::Followed {
                                boosts: true,
                                likes: false
                            }),
                            true
                        ),
                    ]),
                ]),
                TQ::Arg(
                    Arg::Contains(WithContains::Title, "Plume is amazing",),
                    false
                ),
            ])
        );

        let lists = TimelineQuery::parse(
            r#"blog in a or author in b include likes or license in c or tags in d or lang in e "#,
        )
        .unwrap();
        assert_eq!(
            lists.0,
            TQ::Or(vec![
                TQ::Arg(Arg::In(WithList::Blog, List::List("a"),), false),
                TQ::Arg(
                    Arg::In(
                        WithList::Author {
                            boosts: true,
                            likes: true
                        },
                        List::List("b"),
                    ),
                    false
                ),
                TQ::Arg(Arg::In(WithList::License, List::List("c"),), false),
                TQ::Arg(Arg::In(WithList::Tags, List::List("d"),), false),
                TQ::Arg(Arg::In(WithList::Lang, List::List("e"),), false),
            ])
        );

        let contains = TimelineQuery::parse(
            r#"title contains a or subtitle contains b or content contains c"#,
        )
        .unwrap();
        assert_eq!(
            contains.0,
            TQ::Or(vec![
                TQ::Arg(Arg::Contains(WithContains::Title, "a"), false),
                TQ::Arg(Arg::Contains(WithContains::Subtitle, "b"), false),
                TQ::Arg(Arg::Contains(WithContains::Content, "c"), false),
            ])
        );

        let booleans = TimelineQuery::parse(
            r#"followed include like exclude reshares and has_cover and local and all"#,
        )
        .unwrap();
        assert_eq!(
            booleans.0,
            TQ::And(vec![
                TQ::Arg(
                    Arg::Boolean(Bool::Followed {
                        boosts: false,
                        likes: true
                    }),
                    false
                ),
                TQ::Arg(Arg::Boolean(Bool::HasCover), false),
                TQ::Arg(Arg::Boolean(Bool::Local), false),
                TQ::Arg(Arg::Boolean(Bool::All), false),
            ])
        );
    }

    #[test]
    fn test_rejection_parser() {
        let missing_and_or = TimelineQuery::parse(r#"followed or has_cover local"#).unwrap_err();
        assert_eq!(
            missing_and_or,
            QueryError::SyntaxError(
                22,
                5,
                "Syntax Error: Expected on of 'or' or 'and', got 'local'".to_owned()
            )
        );

        let unbalanced_parent =
            TimelineQuery::parse(r#"followed and (has_cover or local"#).unwrap_err();
        assert_eq!(unbalanced_parent, QueryError::UnexpectedEndOfQuery);

        let missing_and_or_in_par =
            TimelineQuery::parse(r#"(title contains "abc def" followed)"#).unwrap_err();
        assert_eq!(
            missing_and_or_in_par,
            QueryError::SyntaxError(
                26,
                8,
                "Syntax Error: Expected ')', got 'followed'".to_owned()
            )
        );

        let expect_in = TimelineQuery::parse(r#"lang contains abc"#).unwrap_err();
        assert_eq!(
            expect_in,
            QueryError::SyntaxError(
                5,
                8,
                "Syntax Error: Expected 'in', got 'contains'".to_owned()
            )
        );

        let expect_contains = TimelineQuery::parse(r#"title in abc"#).unwrap_err();
        assert_eq!(
            expect_contains,
            QueryError::SyntaxError(
                6,
                2,
                "Syntax Error: Expected 'contains', got 'in'".to_owned()
            )
        );

        let expect_keyword = TimelineQuery::parse(r#"not_a_field contains something"#).unwrap_err();
        assert_eq!(
            expect_keyword,
            QueryError::SyntaxError(
                0,
                11,
                "Syntax Error: Expected one of 'blog', \
'author', 'license', 'tags', 'lang', 'title', 'subtitle', 'content', 'followed', 'has_cover', \
'local' or 'all', got 'not_a_field'"
                    .to_owned()
            )
        );

        let expect_bracket_or_comma = TimelineQuery::parse(r#"lang in [en ["#).unwrap_err();
        assert_eq!(
            expect_bracket_or_comma,
            QueryError::SyntaxError(
                12,
                1,
                "Syntax Error: Expected one of ']' or ',', \
                 got '['"
                    .to_owned()
            )
        );

        let expect_bracket = TimelineQuery::parse(r#"lang in )abc"#).unwrap_err();
        assert_eq!(
            expect_bracket,
            QueryError::SyntaxError(
                8,
                1,
                "Syntax Error: Expected one of [list, of, words] or list_name, \
                 got ')'"
                    .to_owned()
            )
        );

        let expect_word = TimelineQuery::parse(r#"title contains ,"#).unwrap_err();
        assert_eq!(
            expect_word,
            QueryError::SyntaxError(15, 1, "Syntax Error: Expected any word, got ','".to_owned())
        );

        let got_bracket = TimelineQuery::parse(r#"lang in []"#).unwrap_err();
        assert_eq!(
            got_bracket,
            QueryError::SyntaxError(9, 1, "Syntax Error: Expected any word, got ']'".to_owned())
        );

        let got_par = TimelineQuery::parse(r#"lang in [a, ("#).unwrap_err();
        assert_eq!(
            got_par,
            QueryError::SyntaxError(12, 1, "Syntax Error: Expected any word, got '('".to_owned())
        );
    }

    #[test]
    fn test_list_used_lists() {
        let q = TimelineQuery::parse(r#"lang in [fr, en] and blog in blogs or author in my_fav_authors or tags in hashtag and lang in spoken or license in copyleft"#)
            .unwrap();
        let used_lists = q.list_used_lists();
        assert_eq!(
            used_lists,
            vec![
                ("blogs".to_owned(), ListType::Blog),
                ("my_fav_authors".to_owned(), ListType::User),
                ("hashtag".to_owned(), ListType::Word),
                ("spoken".to_owned(), ListType::Prefix),
                ("copyleft".to_owned(), ListType::Word),
            ]
        );
    }
}
