use gettextrs::gettext;
use heck::CamelCase;
use openssl::rand::rand_bytes;
use pulldown_cmark::{Event, Parser, Options, Tag, html};
use rocket::{
    http::uri::Uri,
    response::{Redirect, Flash}
};
use std::collections::HashSet;

/// Generates an hexadecimal representation of 32 bytes of random data
pub fn random_hex() -> String {
	let mut bytes = [0; 32];
    rand_bytes(&mut bytes).expect("Error while generating client id");
    bytes.into_iter().fold(String::new(), |res, byte| format!("{}{:x}", res, byte))
}

/// Remove non alphanumeric characters and CamelCase a string
pub fn make_actor_id(name: String) -> String {
    name.as_str()
        .to_camel_case()
        .to_string()
        .chars()
        .filter(|c| c.is_alphanumeric())
        .collect()
}

pub fn requires_login(message: &str, url: Uri) -> Flash<Redirect> {
    Flash::new(Redirect::to(format!("/login?m={}", gettext(message.to_string()))), "callback", url.to_string())
}

#[derive(Debug)]
enum State {
    Mention,
    Hashtag,
    Word,
    Ready,
}

/// Returns (HTML, mentions, hashtags)
pub fn md_to_html(md: &str) -> (String, HashSet<String>, HashSet<String>) {
    let parser = Parser::new_ext(md, Options::all());

    let (parser, mentions, hashtags): (Vec<Vec<Event>>, Vec<Vec<String>>, Vec<Vec<String>>) = parser.map(|evt| match evt {
        Event::Text(txt) => {
            let (evts, _, _, _, new_mentions, new_hashtags) = txt.chars().fold((vec![], State::Ready, String::new(), 0, vec![], vec![]), |(mut events, state, text_acc, n, mut mentions, mut hashtags), c| {
                match state {
                    State::Mention => {
                        let char_matches = c.is_alphanumeric() || c == '@' || c == '.' || c == '-' || c == '_';
                        if char_matches && (n < (txt.chars().count() - 1)) {
                            (events, State::Mention, text_acc + c.to_string().as_ref(), n + 1, mentions, hashtags)
                        } else {
                            let mention = if char_matches {
                                text_acc + c.to_string().as_ref()
                            } else {
                                text_acc
                            };
                            let short_mention = mention.clone();
                            let short_mention = short_mention.splitn(1, '@').nth(0).unwrap_or("");
                            let link = Tag::Link(format!("/@/{}/", mention).into(), short_mention.to_string().into());

                            mentions.push(mention);
                            events.push(Event::Start(link.clone()));
                            events.push(Event::Text(format!("@{}", short_mention).into()));
                            events.push(Event::End(link));

                            (events, State::Ready, c.to_string(), n + 1, mentions, hashtags)
                        }
                    }
                    State::Hashtag => {
                        let char_matches = c.is_alphanumeric();
                        if char_matches && (n < (txt.chars().count() -1)) {
                            (events, State::Hashtag, text_acc + c.to_string().as_ref(), n+1, mentions, hashtags)
                        } else {
                            let hashtag = if char_matches {
                                text_acc + c.to_string().as_ref()
                            } else {
                                text_acc
                            };
                            let link = Tag::Link(format!("/tag/{}", hashtag.to_camel_case()).into(), hashtag.to_string().into());

                            hashtags.push(hashtag.clone());
                            events.push(Event::Start(link.clone()));
                            events.push(Event::Text(format!("#{}", hashtag).into()));
                            events.push(Event::End(link));

                            (events, State::Ready, c.to_string(), n + 1, mentions, hashtags)
                        }
                    }
                    State::Ready => {
                        if c == '@' {
                            events.push(Event::Text(text_acc.into()));
                            (events, State::Mention, String::new(), n + 1, mentions, hashtags)
                        } else if c == '#' {
                            events.push(Event::Text(text_acc.into()));
                            (events, State::Hashtag, String::new(), n + 1, mentions, hashtags)
                        } else if c.is_alphanumeric() {
                            if n >= (txt.chars().count() - 1) { // Add the text after at the end, even if it is not followed by a mention.
                                events.push(Event::Text((text_acc.clone() + c.to_string().as_ref()).into()))
                            }
                            (events, State::Word, text_acc + c.to_string().as_ref(), n + 1, mentions, hashtags)
                        } else {
                            if n >= (txt.chars().count() - 1) { // Add the text after at the end, even if it is not followed by a mention.
                                events.push(Event::Text((text_acc.clone() + c.to_string().as_ref()).into()))
                            }
                            (events, State::Ready, text_acc + c.to_string().as_ref(), n + 1, mentions, hashtags)
                        }
                    }
                    State::Word => {
                        if c.is_alphanumeric() {
                            if n >= (txt.chars().count() - 1) { // Add the text after at the end, even if it is not followed by a mention.
                                events.push(Event::Text((text_acc.clone() + c.to_string().as_ref()).into()))
                            }
                            (events, State::Word, text_acc + c.to_string().as_ref(), n + 1, mentions, hashtags)
                        } else {
                            if n >= (txt.chars().count() - 1) { // Add the text after at the end, even if it is not followed by a mention.
                                events.push(Event::Text((text_acc.clone() + c.to_string().as_ref()).into()))
                            }
                            (events, State::Ready, text_acc + c.to_string().as_ref(), n + 1, mentions, hashtags)
                        }
                    }
                }
            });
            (evts, new_mentions, new_hashtags)
        },
        _ => (vec![evt], vec![], vec![])
    }).fold((vec![],vec![],vec![]), |(mut parser, mut mention, mut hashtag), (p, m, h)| {
        parser.push(p);
        mention.push(m);
        hashtag.push(h);
        (parser, mention, hashtag)
    });
    let parser = parser.into_iter().flatten();
    let mentions = mentions.into_iter().flatten().map(|m| String::from(m.trim()));
    let hashtags = hashtags.into_iter().flatten().map(|h| String::from(h.trim()));

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
            ("      @spaces     ", vec!["spaces"]),
            ("not_a@mention", vec![]),
        ];

        for (md, mentions) in tests {
            assert_eq!(md_to_html(md).1, mentions.into_iter().map(|s| s.to_string()).collect::<HashSet<String>>());
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
            ("      #spaces     ", vec!["spaces"]),
            ("not_a#hashtag", vec![]),
        ];

        for (md, mentions) in tests {
            assert_eq!(md_to_html(md).2, mentions.into_iter().map(|s| s.to_string()).collect::<HashSet<String>>());
        }
    }
}
