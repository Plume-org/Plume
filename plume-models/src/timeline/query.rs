use plume_common::activity_pub::inbox::WithInbox;
use posts::Post;
use {Connection, Result};

use super::Timeline;

#[derive(Debug)]
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

#[derive(Debug, Clone, Copy)]
enum Token<'a> {
    LParent(usize),
    RParent(usize),
    LBracket(usize),
    RBracket(usize),
    Comma(usize),
    Word(usize, usize, &'a str),
}

impl<'a> Token<'a> {
    fn get_word(&self) -> Option<&'a str> {
        match self {
            Token::Word(_, _, s) => Some(s),
            _ => None,
        }
    }

    fn get_pos(&self) -> (usize, usize) {
        match self {
            Token::Word(a, b, _) => (*a, *b),
            Token::LParent(a)
            | Token::RParent(a)
            | Token::LBracket(a)
            | Token::RBracket(a)
            | Token::Comma(a) => (*a, 0),
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
                    *$state = Some(Token::Word($i,0,&""));
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

#[derive(Debug, Clone)]
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

#[derive(Debug, Clone)]
enum Arg<'a> {
    In(WithList, List<'a>),
    Contains(WithContain, &'a str),
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

#[derive(Debug, Clone)]
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

#[derive(Debug, Clone)]
enum WithContain {
    Title,
    Subtitle,
    Content,
}

impl WithContain {
    pub fn matches(&self, post: &Post, value: &str) -> Result<bool> {
        match self {
            WithContain::Title => Ok(post.title.contains(value)),
            WithContain::Subtitle => Ok(post.subtitle.contains(value)),
            WithContain::Content => Ok(post.content.contains(value)),
        }
    }
}

#[derive(Debug, Clone)]
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

#[derive(Debug, Clone)]
enum List<'a> {
    List(&'a str),       //=> list_id
    Array(Vec<&'a str>), //=>store as anonymous list
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
    match stream.get(0).and_then(Token::get_word)? {
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
                        "title" => WithContain::Title,
                        "subtitle" => WithContain::Subtitle,
                        "content" => WithContain::Content,
                        _ => unreachable!(),
                    },
                    w,
                ),
            )),
            (t, _) => t.get_error(Token::Word(0, 0, "'contains'")),
        },
        s @ "followed" | s @ "has_cover" | s @ "local" | s @ "all" => match s {
            "followed" => Ok((&stream[1..], Arg::Boolean(Bool::Followed))),
            "has_cover" => Ok((&stream[1..], Arg::Boolean(Bool::HasCover))),
            "local" => Ok((&stream[1..], Arg::Boolean(Bool::Local))),
            "all" => Ok((&stream[1..], Arg::Boolean(Bool::All))),
            _ => unreachable!(),
        },
        t => Token::Word(1, 1, t).get_error(Token::Word(
            0,
            0,
            r#"one of 'blog', 'author', 'license', 'tags', 'lang',
'title', 'subtitle', 'content', 'followed', 'has_cover', 'local' or 'all'"#,
        )),
    }
}

fn parse_l<'a, 'b>(stream: &'b [Token<'a>]) -> QueryResult<(&'b [Token<'a>], List<'a>)> {
    match stream.get(0)? {
        Token::LBracket(_) => {
            let (left, list) = parse_m(&stream[1..])?;
            match left.get(0)? {
                Token::RBracket(_) => Ok((&left[1..], List::Array(list))),
                t => t.get_error(Token::RBracket(0)),
            }
        }
        Token::Word(_, _, list) => Ok((&stream[1..], List::List(list))),
        t => t.get_error(Token::Word(0, 0, "one of '[' of <list name>")),
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

pub struct TimelineQuery<'a>(TQ<'a>);

impl<'a> TimelineQuery<'a> {
    pub fn parse(query: &'a str) -> QueryResult<Self> {
        parse_s(&lex(query))
            .and_then(|(left, res)| {
                if left.is_empty() {
                    Ok(res)
                } else {
                    left[0].get_error(Token::Word(0, 0, "expected on of 'or' or 'and'"))
                }
            })
            .map(TimelineQuery)
    }

    pub fn matches(&self, conn: &Connection, timeline: &Timeline, post: &Post) -> Result<bool> {
        self.0.matches(conn, timeline, post)
    }
}
