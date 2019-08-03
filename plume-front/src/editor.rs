use stdweb::{
    unstable::{TryFrom, TryInto},
    web::{event::*, html_element::*, *},
};
use CATALOG;

macro_rules! mv {
    ( $( $var:ident ),* => $exp:expr ) => {
        {
            $( let $var = $var.clone(); )*
            $exp
        }
    }
}

fn get_elt_value(id: &'static str) -> String {
    let elt = document().get_element_by_id(id).unwrap();
    let inp: Result<InputElement, _> = elt.clone().try_into();
    let select: Result<SelectElement, _> = elt.clone().try_into();
    let textarea: Result<TextAreaElement, _> = elt.try_into();
    let res = inp.map(|i| i.raw_value()).unwrap_or_else(|_| {
        textarea
            .map(|t| t.value())
            .unwrap_or_else(|_| select.unwrap().value().unwrap_or_default())
    });
    res
}

fn no_return(evt: KeyDownEvent) {
    if evt.key() == "Enter" {
        evt.prevent_default();
    }
}

#[derive(Debug)]
pub enum EditorError {
    NoneError,
    DOMError,
    TypeError,
}

impl From<std::option::NoneError> for EditorError {
    fn from(_: std::option::NoneError) -> Self {
        EditorError::NoneError
    }
}
impl From<stdweb::web::error::InvalidCharacterError> for EditorError {
    fn from(_: stdweb::web::error::InvalidCharacterError) -> Self {
        EditorError::DOMError
    }
}
impl From<stdweb::private::TODO> for EditorError {
    fn from(_: stdweb::private::TODO) -> Self {
        EditorError::DOMError
    }
}
impl From<stdweb::private::ConversionError> for EditorError {
    fn from(_: stdweb::private::ConversionError) -> Self {
        EditorError::TypeError
    }
}

fn filter_paste(elt: &Element) {
    // Only insert text when pasting something
    js! {
        @{&elt}.addEventListener("paste", function (evt) {
            evt.preventDefault();
            document.execCommand("insertText", false, evt.clipboardData.getData("text"));
        });
    };
}

pub fn init() -> Result<(), EditorError> {
    // Check if the user wants to use the basic editor
    if let Some(basic_editor) = window().local_storage().get("basic-editor") {
        if basic_editor == "true" {
            if let Some(editor) = document().get_element_by_id("plume-fallback-editor") {
                if let Ok(Some(title_label)) = document().query_selector("label[for=title]") {
                    let editor_button = document().create_element("a")?;
                    js! { @{&editor_button}.href = "#"; }
                    editor_button.add_event_listener(|_: ClickEvent| {
                        window().local_storage().remove("basic-editor");
                        window().history().go(0).ok(); // refresh
                    });
                    editor_button.append_child(
                        &document().create_text_node(&i18n!(CATALOG, "Open the rich text editor")),
                    );
                    editor.insert_before(&editor_button, &title_label).ok();
                    return Ok(());
                }
            }
        }
    }

    // If we didn't returned above
    init_editor()
}

fn init_editor() -> Result<(), EditorError> {
    if let Some(ed) = document().get_element_by_id("plume-editor") {
        document().body()?.set_attribute("id", "editor")?;

        let aside = document().get_element_by_id("plume-editor-aside")?;

        // Show the editor
        js! {
            @{&ed}.style.display = "grid";
            @{&aside}.style.display = "block";
        };
        // And hide the HTML-only fallback
        let old_ed = document().get_element_by_id("plume-fallback-editor")?;
        js! {
            @{&old_ed}.style.display = "none";
        };

        // And pre-fill the new editor with this values
        let title = document().get_element_by_id("editor-title")?;
        let subtitle = document().get_element_by_id("editor-subtitle")?;
        let content = document().get_element_by_id("editor-default-paragraph")?;

        title.add_event_listener(no_return);
        subtitle.add_event_listener(no_return);

        filter_paste(&title);
        filter_paste(&subtitle);
        filter_paste(&content);

        // character counter
        content.add_event_listener(mv!(content => move |_: KeyDownEvent| {
            window().set_timeout(mv!(content => move || {
                if let Some(e) = document().get_element_by_id("char-count") {
                    let count = chars_left("#plume-fallback-editor", &content).unwrap_or_default();
                    let text = i18n!(CATALOG, "Around {} characters left"; count);
                    HtmlElement::try_from(e).map(|e| {
                        js!{@{e}.innerText = @{text}};
                    }).ok();
                };
            }), 0);
        }));

        document()
            .get_element_by_id("publish")?
            .add_event_listener(|_: ClickEvent| {
                let publish_page = document().get_element_by_id("publish-page").unwrap();
                let options_page = document().get_element_by_id("options-page").unwrap();
                js! {
                    @{&options_page}.style.display = "none";
                    @{&publish_page}.style.display = "flex";
                };
            });

        document()
            .get_element_by_id("cancel-publish")?
            .add_event_listener(|_: ClickEvent| {
                let publish_page = document().get_element_by_id("publish-page").unwrap();
                let options_page = document().get_element_by_id("options-page").unwrap();
                js! {
                    @{&publish_page}.style.display = "none";
                    @{&options_page}.style.display = "flex";
                };
            });

        document()
            .get_element_by_id("confirm-publish")?
            .add_event_listener(|_: ClickEvent| {
                save(false);
            });

        document()
            .get_element_by_id("save-draft")?
            .add_event_listener(|_: ClickEvent| {
                save(true);
            });

        show_errors();
        setup_close_button();
    }
    Ok(())
}

fn save(is_draft: bool) {
    let req = XmlHttpRequest::new();
    if bool::try_from(js! { return window.editing }).unwrap_or(false) {
        req.open(
            "PUT",
            &format!(
                "/api/v1/posts/{}",
                i32::try_from(js! { return window.post_id }).unwrap()
            ),
        )
        .unwrap();
    } else {
        req.open("POST", "/api/v1/posts").unwrap();
    }
    req.set_request_header("Accept", "application/json")
        .unwrap();
    req.set_request_header("Content-Type", "application/json")
        .unwrap();
    req.set_request_header(
        "Authorization",
        &format!(
            "Bearer {}",
            String::try_from(js! { return window.api_token }).unwrap()
        ),
    )
    .unwrap();
    let req_clone = req.clone();
    req.add_event_listener(move |_: ProgressLoadEvent| {
        if let Ok(Some(res)) = req_clone.response_text() {
            serde_json::from_str(&res)
                .map(|res: plume_api::posts::PostData| {
                    let url = res.url;
                    js! {
                        window.location.href = @{url};
                    };
                })
                .map_err(|_| {
                    let json: serde_json::Value = serde_json::from_str(&res).unwrap();
                    window().alert(&format!(
                        "Error: {}",
                        json["error"].as_str().unwrap_or_default()
                    ));
                })
                .ok();
        }
    });
    let data = plume_api::posts::NewPostData {
        title: HtmlElement::try_from(document().get_element_by_id("editor-title").unwrap())
            .unwrap()
            .inner_text(),
        subtitle: document()
            .get_element_by_id("editor-subtitle")
            .map(|s| HtmlElement::try_from(s).unwrap().inner_text()),
        source: HtmlElement::try_from(
            document()
                .get_element_by_id("editor-default-paragraph")
                .unwrap(),
        )
        .unwrap()
        .inner_text(),
        author: String::new(), // it is ignored anyway (TODO: remove it ??)
        blog_id: i32::try_from(js! { return window.blog_id }).ok(),
        published: Some(!is_draft),
        creation_date: None,
        license: Some(get_elt_value("license")),
        tags: Some(
            get_elt_value("tags")
                .split(',')
                .map(|t| t.trim().to_string())
                .filter(|t| !t.is_empty())
                .collect(),
        ),
        cover_id: get_elt_value("cover").parse().ok(),
    };
    let json = serde_json::to_string(&data).unwrap();
    req.send_with_string(&json).unwrap();
}

fn setup_close_button() {
    if let Some(button) = document().get_element_by_id("close-editor") {
        button.add_event_listener(|_: ClickEvent| {
            window()
                .local_storage()
                .insert("basic-editor", "true")
                .unwrap();
            window().history().go(0).unwrap(); // Refresh the page
        });
    }
}

fn show_errors() {
    if let Ok(Some(header)) = document().query_selector("header") {
        let list = document().create_element("header").unwrap();
        list.class_list().add("messages").unwrap();
        for error in document().query_selector_all("p.error").unwrap() {
            error
                .parent_element()
                .unwrap()
                .remove_child(&error)
                .unwrap();
            list.append_child(&error);
        }
        header
            .parent_element()
            .unwrap()
            .insert_before(&list, &header.next_sibling().unwrap())
            .unwrap();
    }
}

fn chars_left(selector: &str, content: &Element) -> Option<i32> {
    match document().query_selector(selector) {
        Ok(Some(form)) => HtmlElement::try_from(form).ok().and_then(|form| {
            if let Some(len) = form
                .get_attribute("content-size")
                .and_then(|s| s.parse::<i32>().ok())
            {
                (js! {
                    let x = encodeURIComponent(@{content}.innerHTML)
                        .replace(/%20/g, "+")
                        .replace(/%0A/g, "%0D%0A")
                        .replace(new RegExp("[!'*()]", "g"), "XXX") // replace exceptions of encodeURIComponent with placeholder
                        .length + 2;
                    console.log(x);
                    return x;
                })
                .try_into()
                .map(|c: i32| len - c)
                .ok()
            } else {
                None
            }
        }),
        _ => None,
    }
}
