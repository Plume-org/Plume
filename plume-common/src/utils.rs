use heck::CamelCase;
use openssl::rand::rand_bytes;
use pulldown_cmark::{Event, Parser, Options, Tag, html};
use rocket::{
    http::uri::Uri,
    response::{Redirect, Flash}
};
use std::borrow::Cow;
use std::collections::HashSet;

/// Generates an hexadecimal representation of 32 bytes of random data
pub fn random_hex() -> String {
	let mut bytes = [0; 32];
    rand_bytes(&mut bytes).expect("Error while generating client id");
    bytes.iter().fold(String::new(), |res, byte| format!("{}{:x}", res, byte))
}

/// Remove non alphanumeric characters and CamelCase a string
pub fn make_actor_id(name: &str) -> String {
    name.to_camel_case()
        .chars()
        .filter(|c| c.is_alphanumeric())
        .collect()
}

/**
* Redirects to the login page with a given message.
*
* Note that the message should be translated before passed to this function.
*/
pub fn requires_login<T: Into<Uri<'static>>>(message: &str, url: T) -> Flash<Redirect> {
    Flash::new(Redirect::to(format!("/login?m={}", Uri::percent_encode(message))), "callback", url.into().to_string())
}

#[derive(Debug)]
enum State {
    Mention,
    Hashtag,
    Word,
    Ready,
}

/// Returns (HTML, mentions, hashtags)
pub fn md_to_html(md: &str, base_url: &str) -> (String, HashSet<String>, HashSet<String>) {
    let parser = Parser::new_ext(md, Options::all());

    let (parser, mentions, hashtags): (Vec<Event>, Vec<String>, Vec<String>) = parser
                                    .scan(None, |state: &mut Option<String>, evt|{
                                        let (s, res) = match evt {
                                            Event::Text(txt) => match state.take() {
                                                Some(mut prev_txt) => {
                                                    prev_txt.push_str(&txt);
                                                    (Some(prev_txt), vec![])
                                                },
                                                None => {
                                                    (Some(txt.into_owned()), vec![])
                                                }
                                            },
                                            e => match state.take() {
                                                Some(prev) => (None, vec![Event::Text(Cow::Owned(prev)), e]),
                                                None => (None, vec![e]),
                                            }
                                        };
                                        *state = s;
                                        Some(res)
                                    })
                                    .flat_map(|v| v.into_iter())
                                    .map(|evt| match evt {
        Event::Text(txt) => {
            let (evts, _, _, _, new_mentions, new_hashtags) = txt.chars().fold((vec![], State::Ready, String::new(), 0, vec![], vec![]), |(mut events, state, mut text_acc, n, mut mentions, mut hashtags), c| {
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
                            let short_mention = mention.splitn(1, '@').nth(0).unwrap_or("");
                            let link = Tag::Link(format!("//{}/@/{}/", base_url, &mention).into(), short_mention.to_owned().into());

                            mentions.push(mention.clone());
                            events.push(Event::Start(link.clone()));
                            events.push(Event::Text(format!("@{}", &short_mention).into()));
                            events.push(Event::End(link));

                            (events, State::Ready, c.to_string(), n + 1, mentions, hashtags)
                        }
                    }
                    State::Hashtag => {
                        let char_matches = c.is_alphanumeric() || "-_".contains(c);
                        if char_matches && (n < (txt.chars().count() -1)) {
                            text_acc.push(c);
                            (events, State::Hashtag, text_acc, n+1, mentions, hashtags)
                        } else {
                            if char_matches {
                                text_acc.push(c);
                            }
                            let hashtag = text_acc;
                            let link = Tag::Link(format!("//{}/tag/{}", base_url, &hashtag.to_camel_case()).into(), hashtag.to_owned().into());

                            hashtags.push(hashtag.clone());
                            events.push(Event::Start(link.clone()));
                            events.push(Event::Text(format!("#{}", &hashtag).into()));
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
                            text_acc.push(c);
                            if n >= (txt.chars().count() - 1) { // Add the text after at the end, even if it is not followed by a mention.
                                events.push(Event::Text(text_acc.clone().into()))
                            }
                            (events, State::Word, text_acc, n + 1, mentions, hashtags)
                        } else {
                            text_acc.push(c);
                            if n >= (txt.chars().count() - 1) { // Add the text after at the end, even if it is not followed by a mention.
                                events.push(Event::Text(text_acc.clone().into()))
                            }
                            (events, State::Ready, text_acc, n + 1, mentions, hashtags)
                        }
                    }
                    State::Word => {
                        text_acc.push(c);
                        if c.is_alphanumeric() {
                            if n >= (txt.chars().count() - 1) { // Add the text after at the end, even if it is not followed by a mention.
                                events.push(Event::Text(text_acc.clone().into()))
                            }
                            (events, State::Word, text_acc, n + 1, mentions, hashtags)
                        } else {
                            if n >= (txt.chars().count() - 1) { // Add the text after at the end, even if it is not followed by a mention.
                                events.push(Event::Text(text_acc.clone().into()))
                            }
                            (events, State::Ready, text_acc, n + 1, mentions, hashtags)
                        }
                    }
                }
            });
            (evts, new_mentions, new_hashtags)
        },
        _ => (vec![evt], vec![], vec![])
    }).fold((vec![],vec![],vec![]), |(mut parser, mut mention, mut hashtag), (mut p, mut m, mut h)| {
        parser.append(&mut p);
        mention.append(&mut m);
        hashtag.append(&mut h);
        (parser, mention, hashtag)
    });
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
            ("      @spaces     ", vec!["spaces"]),
            ("@is_a@mention", vec!["is_a@mention"]),
            ("not_a@mention", vec![]),
        ];

        for (md, mentions) in tests {
            assert_eq!(md_to_html(md, "").1, mentions.into_iter().map(|s| s.to_string()).collect::<HashSet<String>>());
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
            assert_eq!(md_to_html(md, "").2, mentions.into_iter().map(|s| s.to_string()).collect::<HashSet<String>>());
        }
    }
}
