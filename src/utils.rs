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


pub fn md_to_html(md: &str) -> String {
    let parser = Parser::new_ext(md, Options::all());
    let parser = parser.flat_map(|evt| match evt {
        Event::Text(txt) => txt.chars().fold((vec![], false, String::new(), 0), |(mut events, in_mention, text_acc, n), c| {
            if in_mention {
                if (c.is_alphanumeric() || c == '@' || c == '.' || c == '-' || c == '_') && (n < (txt.chars().count() - 1)) {
                    (events, in_mention, text_acc + c.to_string().as_ref(), n + 1)
                } else {
                    let mention = text_acc + c.to_string().as_ref();
                    let short_mention = mention.clone();
                    let short_mention = short_mention.splitn(1, '@').nth(0).unwrap_or("");
                    let link = Tag::Link(format!("/@/{}/", mention).into(), short_mention.to_string().into());

                    events.push(Event::Start(link.clone()));
                    events.push(Event::Text(format!("@{}", short_mention).into()));
                    events.push(Event::End(link));

                    (events, false, c.to_string(), n + 1)
                }
            } else {
                if c == '@' {
                    events.push(Event::Text(text_acc.into()));
                    (events, true, String::new(), n + 1)
                } else {
                    if n >= (txt.chars().count() - 1) { // Add the text after at the end, even if it is not followed by a mention.
                        events.push(Event::Text((text_acc.clone() + c.to_string().as_ref()).into()))
                    }
                    (events, in_mention, text_acc + c.to_string().as_ref(), n + 1)
                }
            }
        }).0,
        _ => vec![evt]
    });
    let mut buf = String::new();
    html::push_html(&mut buf, parser);
    buf

    // let root = parse_document(&arena, md, &ComrakOptions{
    //     smart: true,
    //     safe: true,
    //     ext_strikethrough: true,
    //     ext_tagfilter: true,
    //     ext_table: true,
    //     // ext_autolink: true,
    //     ext_tasklist: true,
    //     ext_superscript: true,
    //     ext_header_ids: Some("title".to_string()),
    //     ext_footnotes: true,
    //     ..ComrakOptions::default()
    // });
}
