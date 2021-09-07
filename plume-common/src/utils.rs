use heck::CamelCase;
use openssl::rand::rand_bytes;
use pulldown_cmark::{html, CodeBlockKind, CowStr, Event, LinkType, Options, Parser, Tag};
use regex_syntax::is_word_character;
use rocket::{
    http::uri::Uri,
    response::{Flash, Redirect},
};
use std::collections::HashSet;
use syntect::html::{ClassStyle, ClassedHTMLGenerator};
use syntect::parsing::SyntaxSet;

/// Generates an hexadecimal representation of 32 bytes of random data
pub fn random_hex() -> String {
    let mut bytes = [0; 32];
    rand_bytes(&mut bytes).expect("Error while generating client id");
    bytes
        .iter()
        .fold(String::new(), |res, byte| format!("{}{:x}", res, byte))
}

/// Remove non alphanumeric characters and CamelCase a string
pub fn make_actor_id(name: &str) -> String {
    name.to_camel_case()
        .chars()
        .filter(|c| c.is_alphanumeric())
        .collect()
}

/**
 * Percent-encode characters which are not allowed in IRI path segments.
 *
 * Intended to be used for generating Post ap_url.
 */
pub fn iri_percent_encode_seg(segment: &str) -> String {
    segment.chars().map(iri_percent_encode_seg_char).collect()
}

pub fn iri_percent_encode_seg_char(c: char) -> String {
    if c.is_alphanumeric() {
        c.to_string()
    } else {
        match c {
            '-'
            | '.'
            | '_'
            | '~'
            | '\u{A0}'..='\u{D7FF}'
            | '\u{20000}'..='\u{2FFFD}'
            | '\u{30000}'..='\u{3FFFD}'
            | '\u{40000}'..='\u{4FFFD}'
            | '\u{50000}'..='\u{5FFFD}'
            | '\u{60000}'..='\u{6FFFD}'
            | '\u{70000}'..='\u{7FFFD}'
            | '\u{80000}'..='\u{8FFFD}'
            | '\u{90000}'..='\u{9FFFD}'
            | '\u{A0000}'..='\u{AFFFD}'
            | '\u{B0000}'..='\u{BFFFD}'
            | '\u{C0000}'..='\u{CFFFD}'
            | '\u{D0000}'..='\u{DFFFD}'
            | '\u{E0000}'..='\u{EFFFD}'
            | '!'
            | '$'
            | '&'
            | '\''
            | '('
            | ')'
            | '*'
            | '+'
            | ','
            | ';'
            | '='
            | ':'
            | '@' => c.to_string(),
            _ => {
                let s = c.to_string();
                Uri::percent_encode(&s).to_string()
            }
        }
    }
}

/**
* Redirects to the login page with a given message.
*
* Note that the message should be translated before passed to this function.
*/
pub fn requires_login<T: Into<Uri<'static>>>(message: &str, url: T) -> Flash<Redirect> {
    Flash::new(
        Redirect::to(format!("/login?m={}", Uri::percent_encode(message))),
        "callback",
        url.into().to_string(),
    )
}

#[derive(Debug)]
enum State {
    Mention,
    Hashtag,
    Word,
    Ready,
}

fn to_inline(tag: Tag<'_>) -> Tag<'_> {
    match tag {
        Tag::Heading(_) | Tag::Table(_) | Tag::TableHead | Tag::TableRow | Tag::TableCell => {
            Tag::Paragraph
        }
        Tag::Image(typ, url, title) => Tag::Link(typ, url, title),
        t => t,
    }
}
struct HighlighterContext {
    content: Vec<String>,
}
#[allow(clippy::unnecessary_wraps)]
fn highlight_code<'a>(
    context: &mut Option<HighlighterContext>,
    evt: Event<'a>,
) -> Option<Vec<Event<'a>>> {
    match evt {
        Event::Start(Tag::CodeBlock(kind)) => {
            match &kind {
                CodeBlockKind::Fenced(lang) if !lang.is_empty() => {
                    *context = Some(HighlighterContext { content: vec![] });
                }
                _ => {}
            }
            Some(vec![Event::Start(Tag::CodeBlock(kind))])
        }
        Event::End(Tag::CodeBlock(kind)) => {
            let mut result = vec![];
            if let Some(ctx) = context.take() {
                let lang = if let CodeBlockKind::Fenced(lang) = &kind {
                    if lang.is_empty() {
                        unreachable!();
                    } else {
                        lang
                    }
                } else {
                    unreachable!();
                };
                let syntax_set = SyntaxSet::load_defaults_newlines();
                let syntax = syntax_set.find_syntax_by_token(&lang).unwrap_or_else(|| {
                    syntax_set
                        .find_syntax_by_name(&lang)
                        .unwrap_or_else(|| syntax_set.find_syntax_plain_text())
                });
                let mut html = ClassedHTMLGenerator::new_with_class_style(
                    &syntax,
                    &syntax_set,
                    ClassStyle::Spaced,
                );
                for line in ctx.content {
                    html.parse_html_for_line_which_includes_newline(&line);
                }
                let q = html.finalize();
                result.push(Event::Html(q.into()));
            }
            result.push(Event::End(Tag::CodeBlock(kind)));
            *context = None;
            Some(result)
        }
        Event::Text(t) => {
            if let Some(mut c) = context.take() {
                c.content.push(t.to_string());
                *context = Some(c);
                Some(vec![])
            } else {
                Some(vec![Event::Text(t)])
            }
        }
        _ => Some(vec![evt]),
    }
}
#[allow(clippy::unnecessary_wraps)]
fn flatten_text<'a>(state: &mut Option<String>, evt: Event<'a>) -> Option<Vec<Event<'a>>> {
    let (s, res) = match evt {
        Event::Text(txt) => match state.take() {
            Some(mut prev_txt) => {
                prev_txt.push_str(&txt);
                (Some(prev_txt), vec![])
            }
            None => (Some(txt.into_string()), vec![]),
        },
        e => match state.take() {
            Some(prev) => (None, vec![Event::Text(CowStr::Boxed(prev.into())), e]),
            None => (None, vec![e]),
        },
    };
    *state = s;
    Some(res)
}

#[allow(clippy::unnecessary_wraps)]
fn inline_tags<'a>(
    (state, inline): &mut (Vec<Tag<'a>>, bool),
    evt: Event<'a>,
) -> Option<Event<'a>> {
    if *inline {
        let new_evt = match evt {
            Event::Start(t) => {
                let tag = to_inline(t);
                state.push(tag.clone());
                Event::Start(tag)
            }
            Event::End(t) => match state.pop() {
                Some(other) => Event::End(other),
                None => Event::End(t),
            },
            e => e,
        };
        Some(new_evt)
    } else {
        Some(evt)
    }
}

pub type MediaProcessor<'a> = Box<dyn 'a + Fn(i32) -> Option<(String, Option<String>)>>;

fn process_image<'a, 'b>(
    evt: Event<'a>,
    inline: bool,
    processor: &Option<MediaProcessor<'b>>,
) -> Event<'a> {
    if let Some(ref processor) = *processor {
        match evt {
            Event::Start(Tag::Image(typ, id, title)) => {
                if let Some((url, cw)) = id.parse::<i32>().ok().and_then(processor.as_ref()) {
                    if let (Some(cw), false) = (cw, inline) {
                        // there is a cw, and where are not inline
                        Event::Html(CowStr::Boxed(
                            format!(
                                r#"<label for="postcontent-cw-{id}">
  <input type="checkbox" id="postcontent-cw-{id}" checked="checked" class="cw-checkbox">
  <span class="cw-container">
    <span class="cw-text">
        {cw}
    </span>
  <img src="{url}" alt=""#,
                                id = random_hex(),
                                cw = cw,
                                url = url
                            )
                            .into(),
                        ))
                    } else {
                        Event::Start(Tag::Image(typ, CowStr::Boxed(url.into()), title))
                    }
                } else {
                    Event::Start(Tag::Image(typ, id, title))
                }
            }
            Event::End(Tag::Image(typ, id, title)) => {
                if let Some((url, cw)) = id.parse::<i32>().ok().and_then(processor.as_ref()) {
                    if inline || cw.is_none() {
                        Event::End(Tag::Image(typ, CowStr::Boxed(url.into()), title))
                    } else {
                        Event::Html(CowStr::Borrowed(
                            r#""/>
  </span>
</label>"#,
                        ))
                    }
                } else {
                    Event::End(Tag::Image(typ, id, title))
                }
            }
            e => e,
        }
    } else {
        evt
    }
}

#[derive(Default, Debug)]
struct DocumentContext {
    in_code: bool,
    in_link: bool,
}

/// Returns (HTML, mentions, hashtags)
pub fn md_to_html<'a>(
    md: &str,
    base_url: Option<&str>,
    inline: bool,
    media_processor: Option<MediaProcessor<'a>>,
) -> (String, HashSet<String>, HashSet<String>) {
    let base_url = if let Some(base_url) = base_url {
        format!("//{}/", base_url)
    } else {
        "/".to_owned()
    };
    let parser = Parser::new_ext(md, Options::all());

    let (parser, mentions, hashtags): (Vec<Event<'_>>, Vec<String>, Vec<String>) = parser
        // Flatten text because pulldown_cmark break #hashtag in two individual text elements
        .scan(None, flatten_text)
        .flatten()
        .scan(None, highlight_code)
        .flatten()
        .map(|evt| process_image(evt, inline, &media_processor))
        // Ignore headings, images, and tables if inline = true
        .scan((vec![], inline), inline_tags)
        .scan(&mut DocumentContext::default(), |ctx, evt| match evt {
            Event::Start(Tag::CodeBlock(_)) => {
                ctx.in_code = true;
                Some((vec![evt], vec![], vec![]))
            }
            Event::End(Tag::CodeBlock(_)) => {
                ctx.in_code = false;
                Some((vec![evt], vec![], vec![]))
            }
            Event::Start(Tag::Link(_, _, _)) => {
                ctx.in_link = true;
                Some((vec![evt], vec![], vec![]))
            }
            Event::End(Tag::Link(_, _, _)) => {
                ctx.in_link = false;
                Some((vec![evt], vec![], vec![]))
            }
            Event::Text(txt) => {
                let (evts, _, _, _, new_mentions, new_hashtags) = txt.chars().fold(
                    (vec![], State::Ready, String::new(), 0, vec![], vec![]),
                    |(mut events, state, mut text_acc, n, mut mentions, mut hashtags), c| {
                        match state {
                            State::Mention => {
                                let char_matches = c.is_alphanumeric() || "@.-_".contains(c);
                                if char_matches && (n < (txt.chars().count() - 1)) {
                                    text_acc.push(c);
                                    (events, State::Mention, text_acc, n + 1, mentions, hashtags)
                                } else {
                                    if char_matches {
                                        text_acc.push(c)
                                    }
                                    let mention = text_acc;
                                    let short_mention = mention.splitn(1, '@').next().unwrap_or("");
                                    let link = Tag::Link(
                                        LinkType::Inline,
                                        format!("{}@/{}/", base_url, &mention).into(),
                                        short_mention.to_owned().into(),
                                    );

                                    mentions.push(mention.clone());
                                    events.push(Event::Start(link.clone()));
                                    events.push(Event::Text(format!("@{}", &short_mention).into()));
                                    events.push(Event::End(link));

                                    (
                                        events,
                                        State::Ready,
                                        c.to_string(),
                                        n + 1,
                                        mentions,
                                        hashtags,
                                    )
                                }
                            }
                            State::Hashtag => {
                                let char_matches = c == '-' || is_word_character(c);
                                if char_matches && (n < (txt.chars().count() - 1)) {
                                    text_acc.push(c);
                                    (events, State::Hashtag, text_acc, n + 1, mentions, hashtags)
                                } else {
                                    if char_matches {
                                        text_acc.push(c);
                                    }
                                    let hashtag = text_acc;
                                    let link = Tag::Link(
                                        LinkType::Inline,
                                        format!("{}tag/{}", base_url, &hashtag).into(),
                                        hashtag.to_owned().into(),
                                    );

                                    hashtags.push(hashtag.clone());
                                    events.push(Event::Start(link.clone()));
                                    events.push(Event::Text(format!("#{}", &hashtag).into()));
                                    events.push(Event::End(link));

                                    (
                                        events,
                                        State::Ready,
                                        c.to_string(),
                                        n + 1,
                                        mentions,
                                        hashtags,
                                    )
                                }
                            }
                            State::Ready => {
                                if !ctx.in_code && !ctx.in_link && c == '@' {
                                    events.push(Event::Text(text_acc.into()));
                                    (
                                        events,
                                        State::Mention,
                                        String::new(),
                                        n + 1,
                                        mentions,
                                        hashtags,
                                    )
                                } else if !ctx.in_code && !ctx.in_link && c == '#' {
                                    events.push(Event::Text(text_acc.into()));
                                    (
                                        events,
                                        State::Hashtag,
                                        String::new(),
                                        n + 1,
                                        mentions,
                                        hashtags,
                                    )
                                } else if c.is_alphanumeric() {
                                    text_acc.push(c);
                                    if n >= (txt.chars().count() - 1) {
                                        // Add the text after at the end, even if it is not followed by a mention.
                                        events.push(Event::Text(text_acc.clone().into()))
                                    }
                                    (events, State::Word, text_acc, n + 1, mentions, hashtags)
                                } else {
                                    text_acc.push(c);
                                    if n >= (txt.chars().count() - 1) {
                                        // Add the text after at the end, even if it is not followed by a mention.
                                        events.push(Event::Text(text_acc.clone().into()))
                                    }
                                    (events, State::Ready, text_acc, n + 1, mentions, hashtags)
                                }
                            }
                            State::Word => {
                                text_acc.push(c);
                                if c.is_alphanumeric() {
                                    if n >= (txt.chars().count() - 1) {
                                        // Add the text after at the end, even if it is not followed by a mention.
                                        events.push(Event::Text(text_acc.clone().into()))
                                    }
                                    (events, State::Word, text_acc, n + 1, mentions, hashtags)
                                } else {
                                    if n >= (txt.chars().count() - 1) {
                                        // Add the text after at the end, even if it is not followed by a mention.
                                        events.push(Event::Text(text_acc.clone().into()))
                                    }
                                    (events, State::Ready, text_acc, n + 1, mentions, hashtags)
                                }
                            }
                        }
                    },
                );
                Some((evts, new_mentions, new_hashtags))
            }
            _ => Some((vec![evt], vec![], vec![])),
        })
        .fold(
            (vec![], vec![], vec![]),
            |(mut parser, mut mention, mut hashtag), (mut p, mut m, mut h)| {
                parser.append(&mut p);
                mention.append(&mut m);
                hashtag.append(&mut h);
                (parser, mention, hashtag)
            },
        );
    let parser = parser.into_iter();
    let mentions = mentions.into_iter().map(|m| String::from(m.trim()));
    let hashtags = hashtags.into_iter().map(|h| String::from(h.trim()));

    // TODO: fetch mentionned profiles in background, if needed

    let mut buf = String::new();
    html::push_html(&mut buf, parser);
    (buf, mentions.collect(), hashtags.collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mentions() {
        let tests = vec![
            ("nothing", vec![]),
            ("@mention", vec!["mention"]),
            ("@mention@instance.tld", vec!["mention@instance.tld"]),
            ("@many @mentions", vec!["many", "mentions"]),
            ("@start with a mentions", vec!["start"]),
            ("mention at @end", vec!["end"]),
            ("between parenthesis (@test)", vec!["test"]),
            ("with some punctuation @test!", vec!["test"]),
            (" @spaces     ", vec!["spaces"]),
            ("@is_a@mention", vec!["is_a@mention"]),
            ("not_a@mention", vec![]),
            ("`@helo`", vec![]),
            ("```\n@hello\n```", vec![]),
            ("[@atmark in link](https://example.org/)", vec![]),
        ];

        for (md, mentions) in tests {
            assert_eq!(
                md_to_html(md, None, false, None).1,
                mentions
                    .into_iter()
                    .map(|s| s.to_string())
                    .collect::<HashSet<String>>()
            );
        }
    }

    #[test]
    fn test_hashtags() {
        let tests = vec![
            ("nothing", vec![]),
            ("#hashtag", vec!["hashtag"]),
            ("#many #hashtags", vec!["many", "hashtags"]),
            ("#start with a hashtag", vec!["start"]),
            ("hashtag at #end", vec!["end"]),
            ("between parenthesis (#test)", vec!["test"]),
            ("with some punctuation #test!", vec!["test"]),
            (" #spaces     ", vec!["spaces"]),
            ("not_a#hashtag", vec![]),
            ("#نرم‌افزار_آزاد", vec!["نرم‌افزار_آزاد"]),
            ("[#hash in link](https://example.org/)", vec![]),
            ("#zwsp\u{200b}inhash", vec!["zwsp"]),
        ];

        for (md, mentions) in tests {
            assert_eq!(
                md_to_html(md, None, false, None).2,
                mentions
                    .into_iter()
                    .map(|s| s.to_string())
                    .collect::<HashSet<String>>()
            );
        }
    }

    #[test]
    fn test_iri_percent_encode_seg() {
        assert_eq!(
            &iri_percent_encode_seg("including whitespace"),
            "including%20whitespace"
        );
        assert_eq!(&iri_percent_encode_seg("%20"), "%2520");
        assert_eq!(&iri_percent_encode_seg("é"), "é");
        assert_eq!(
            &iri_percent_encode_seg("空白入り 日本語"),
            "空白入り%20日本語"
        );
    }

    #[test]
    fn test_inline() {
        assert_eq!(
            md_to_html("# Hello", None, false, None).0,
            String::from("<h1 dir=\"auto\">Hello</h1>\n")
        );
        assert_eq!(
            md_to_html("# Hello", None, true, None).0,
            String::from("<p dir=\"auto\">Hello</p>\n")
        );
    }
}
