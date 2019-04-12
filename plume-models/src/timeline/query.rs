use plume_common::activity_pub::inbox::WithInbox;
use posts::Post;
use {Connection, Result};

use super::Timeline;

#[derive(Debug, Clone, PartialEq)]
pub enum QueryError {
    SyntaxError(usize, usize, String),
    UnexpectedEndOfQuery,
    RuntimeError(String),
}

impl From<std::option::NoneError> for QueryError {
    fn from(_: std::option::NoneError) -> Self {
        QueryError::UnexpectedEndOfQuery
    }
}

pub type QueryResult<T> = std::result::Result<T, QueryError>;

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

    fn get_error<T>(&self, token: Token) -> QueryResult<T> {
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
            return v.to_string();
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
            ' ' if !*$quote => match $state.take() {
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

fn lex(stream: &str) -> Vec<Token> {
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

#[derive(Debug, Clone, PartialEq)]
enum TQ<'a> {
    Or(Vec<TQ<'a>>),
    And(Vec<TQ<'a>>),
    Arg(Arg<'a>, bool),
}

impl<'a> TQ<'a> {
    pub fn matches(&self, conn: &Connection, timeline: &Timeline, post: &Post) -> Result<bool> {
        match self {
            TQ::Or(inner) => inner
                .iter()
                .try_fold(true, |s, e| e.matches(conn, timeline, post).map(|r| s || r)),
            TQ::And(inner) => inner
                .iter()
                .try_fold(true, |s, e| e.matches(conn, timeline, post).map(|r| s && r)),
            TQ::Arg(inner, invert) => Ok(inner.matches(conn, timeline, post)? ^ invert),
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
    pub fn matches(&self, conn: &Connection, timeline: &Timeline, post: &Post) -> Result<bool> {
        match self {
            Arg::In(t, l) => t.matches(conn, post, l),
            Arg::Contains(t, v) => t.matches(post, v),
            Arg::Boolean(t) => t.matches(conn, timeline, post),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
enum WithList {
    Blog,
    Author,
    License,
    Tags,
    Lang,
}

impl WithList {
    pub fn matches(&self, conn: &Connection, post: &Post, list: &List) -> Result<bool> {
        let _ = (conn, post, list); // trick to hide warnings
        unimplemented!()
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
    Followed,
    HasCover,
    Local,
    All,
}

impl Bool {
    pub fn matches(&self, conn: &Connection, timeline: &Timeline, post: &Post) -> Result<bool> {
        match self {
            Bool::Followed => {
                if let Some(user) = timeline.user_id {
                    post.get_authors(conn)?
                        .iter()
                        .try_fold(false, |s, a| a.is_followed_by(conn, user).map(|r| s || r))
                } else {
                    Ok(false)
                }
            }
            Bool::HasCover => Ok(post.cover_id.is_some()),
            Bool::Local => Ok(post.get_blog(conn)?.is_local()),
            Bool::All => Ok(true),
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
    let (left, token) = parse_a(&stream)?;
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
    let (left, token) = parse_b(&stream)?;
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
                None => None?,
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

fn parse_d<'a, 'b>(stream: &'b [Token<'a>]) -> QueryResult<(&'b [Token<'a>], Arg<'a>)> {
    match stream.get(0).map(Token::get_text)? {
        s @ "blog" | s @ "author" | s @ "license" | s @ "tags" | s @ "lang" => {
            match stream.get(1)? {
                Token::Word(_, _, r#in) if r#in == &"in" => {
                    let (left, list) = parse_l(&stream[2..])?;
                    Ok((
                        left,
                        Arg::In(
                            match s {
                                "blog" => WithList::Blog,
                                "author" => WithList::Author,
                                "license" => WithList::License,
                                "tags" => WithList::Tags,
                                "lang" => WithList::Lang,
                                _ => unreachable!(),
                            },
                            list,
                        ),
                    ))
                }
                t => t.get_error(Token::Word(0, 0, "'in'")),
            }
        }
        s @ "title" | s @ "subtitle" | s @ "content" => match (stream.get(1)?, stream.get(2)?) {
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
            "followed" => Ok((&stream[1..], Arg::Boolean(Bool::Followed))),
            "has_cover" => Ok((&stream[1..], Arg::Boolean(Bool::HasCover))),
            "local" => Ok((&stream[1..], Arg::Boolean(Bool::Local))),
            "all" => Ok((&stream[1..], Arg::Boolean(Bool::All))),
            _ => unreachable!(),
        },
        _ => stream.get(0)?.get_error(Token::Word(
            0,
            0,
            "one of 'blog', 'author', 'license', 'tags', 'lang', \
             'title', 'subtitle', 'content', 'followed', 'has_cover', 'local' or 'all'",
        )),
    }
}

fn parse_l<'a, 'b>(stream: &'b [Token<'a>]) -> QueryResult<(&'b [Token<'a>], List<'a>)> {
    match stream.get(0)? {
        Token::LBracket(_) => {
            let (left, list) = parse_m(&stream[1..])?;
            match left.get(0)? {
                Token::RBracket(_) => Ok((&left[1..], List::Array(list))),
                t => t.get_error(Token::Word(0, 0, "one of ']' or ','")),
            }
        }
        Token::Word(_, _, list) => Ok((&stream[1..], List::List(list))),
        t => t.get_error(Token::Word(0, 0, "one of [list, of, words] or list_name")),
    }
}

fn parse_m<'a, 'b>(mut stream: &'b [Token<'a>]) -> QueryResult<(&'b [Token<'a>], Vec<&'a str>)> {
    let mut res: Vec<&str> = Vec::new();
    res.push(match stream.get(0)? {
        Token::Word(_, _, w) => w,
        t => return t.get_error(Token::Word(0, 0, "any word")),
    });
    stream = &stream[1..];
    while let Token::Comma(_) = stream[0] {
        res.push(match stream.get(1)? {
            Token::Word(_, _, w) => w,
            t => return t.get_error(Token::Word(0, 0, "any word")),
        });
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

    pub fn matches(&self, conn: &Connection, timeline: &Timeline, post: &Post) -> Result<bool> {
        self.0.matches(conn, timeline, post)
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
                        TQ::Arg(Arg::Boolean(Bool::Followed), true),
                    ]),
                ]),
                TQ::Arg(
                    Arg::Contains(WithContains::Title, "Plume is amazing",),
                    false
                ),
            ])
        );

        let lists = TimelineQuery::parse(
            r#"blog in a or author in b or license in c or tags in d or lang in e "#,
        )
        .unwrap();
        assert_eq!(
            lists.0,
            TQ::Or(vec![
                TQ::Arg(Arg::In(WithList::Blog, List::List("a"),), false),
                TQ::Arg(Arg::In(WithList::Author, List::List("b"),), false),
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

        let booleans = TimelineQuery::parse(r#"followed and has_cover and local and all"#).unwrap();
        assert_eq!(
            booleans.0,
            TQ::And(vec![
                TQ::Arg(Arg::Boolean(Bool::Followed), false),
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
        assert_eq!(expect_keyword, QueryError::SyntaxError(0, 11, "Syntax Error: Expected one of 'blog', \
'author', 'license', 'tags', 'lang', 'title', 'subtitle', 'content', 'followed', 'has_cover', \
'local' or 'all', got 'not_a_field'".to_owned()));

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
}
