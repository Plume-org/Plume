use gettextrs::gettext;
use heck::CamelCase;
use pulldown_cmark::{Event, Parser, Options, Tag, html};
use rocket::{
    http::uri::Uri,
    response::{Redirect, Flash}
};

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
    Flash::new(Redirect::to(Uri::new(format!("/login?m={}", gettext(message.to_string())))), "callback", url.as_str())
}

/// Returns (HTML, mentions)
pub fn md_to_html(md: &str) -> (String, Vec<String>) {
    let parser = Parser::new_ext(md, Options::all());

    let (parser, mentions): (Vec<Vec<Event>>, Vec<Vec<String>>) = parser.map(|evt| match evt {
        Event::Text(txt) => {
            let (evts, _, _, _, new_mentions) = txt.chars().fold((vec![], false, String::new(), 0, vec![]), |(mut events, in_mention, text_acc, n, mut mentions), c| {
                if in_mention {
                    if (c.is_alphanumeric() || c == '@' || c == '.' || c == '-' || c == '_') && (n < (txt.chars().count() - 1)) {
                        (events, in_mention, text_acc + c.to_string().as_ref(), n + 1, mentions)
                    } else {
                        let mention = text_acc + c.to_string().as_ref();
                        let short_mention = mention.clone();
                        let short_mention = short_mention.splitn(1, '@').nth(0).unwrap_or("");
                        let link = Tag::Link(format!("/@/{}/", mention).into(), short_mention.to_string().into());

                        mentions.push(mention);
                        events.push(Event::Start(link.clone()));
                        events.push(Event::Text(format!("@{}", short_mention).into()));
                        events.push(Event::End(link));

                        (events, false, c.to_string(), n + 1, mentions)
                    }
                } else {
                    if c == '@' {
                        events.push(Event::Text(text_acc.into()));
                        (events, true, String::new(), n + 1, mentions)
                    } else {
                        if n >= (txt.chars().count() - 1) { // Add the text after at the end, even if it is not followed by a mention.
                            events.push(Event::Text((text_acc.clone() + c.to_string().as_ref()).into()))
                        }
                        (events, in_mention, text_acc + c.to_string().as_ref(), n + 1, mentions)
                    }
                }
            });
            (evts, new_mentions)
        },
        _ => (vec![evt], vec![])
    }).unzip();
    let parser = parser.into_iter().flatten();
    let mentions = mentions.into_iter().flatten().map(|m| String::from(m.trim()));

    // TODO: fetch mentionned profiles in background, if needed

    let mut buf = String::new();
    html::push_html(&mut buf, parser);
    (buf, mentions.collect())
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
        ];

        for (md, mentions) in tests {
            assert_eq!(md_to_html(md).1, mentions.into_iter().map(|s| s.to_string()).collect::<Vec<String>>());
        }
    }
}
