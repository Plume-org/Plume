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
    let textarea: Result<TextAreaElement, _> = elt.try_into();
    inp.map(|i| i.raw_value())
        .unwrap_or_else(|_| textarea.unwrap().value())
}

fn set_value<S: AsRef<str>>(id: &'static str, val: S) {
    let elt = document().get_element_by_id(id).unwrap();
    let inp: Result<InputElement, _> = elt.clone().try_into();
    let textarea: Result<TextAreaElement, _> = elt.try_into();
    inp.map(|i| i.set_raw_value(val.as_ref()))
        .unwrap_or_else(|_| textarea.unwrap().set_value(val.as_ref()))
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

        show_errors();
        setup_close_button();
    }
    Ok(())
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

fn init_popup(
    title: &HtmlElement,
    subtitle: &HtmlElement,
    content: &HtmlElement,
    old_ed: &Element,
) -> Result<Element, EditorError> {
    let popup = document().create_element("div")?;
    popup.class_list().add("popup")?;
    popup.set_attribute("id", "publish-popup")?;

    let tags = get_elt_value("tags")
        .split(',')
        .map(str::trim)
        .map(str::to_string)
        .collect::<Vec<_>>();
    let license = get_elt_value("license");
    make_input(&i18n!(CATALOG, "Tags"), "popup-tags", &popup).set_raw_value(&tags.join(", "));
    make_input(&i18n!(CATALOG, "License"), "popup-license", &popup).set_raw_value(&license);

    let cover_label = document().create_element("label")?;
    cover_label.append_child(&document().create_text_node(&i18n!(CATALOG, "Cover")));
    cover_label.set_attribute("for", "cover")?;
    let cover = document().get_element_by_id("cover")?;
    cover.parent_element()?.remove_child(&cover).ok();
    popup.append_child(&cover_label);
    popup.append_child(&cover);

    if let Some(draft_checkbox) = document().get_element_by_id("draft") {
        let draft_label = document().create_element("label")?;
        draft_label.set_attribute("for", "popup-draft")?;

        let draft = document().create_element("input").unwrap();
        js! {
            @{&draft}.id = "popup-draft";
            @{&draft}.name = "popup-draft";
            @{&draft}.type = "checkbox";
            @{&draft}.checked = @{&draft_checkbox}.checked;
        };

        draft_label.append_child(&draft);
        draft_label.append_child(&document().create_text_node(&i18n!(CATALOG, "This is a draft")));
        popup.append_child(&draft_label);
    }

    let button = document().create_element("input")?;
    js! {
        @{&button}.type = "submit";
        @{&button}.value = @{i18n!(CATALOG, "Publish")};
    };
    button.append_child(&document().create_text_node(&i18n!(CATALOG, "Publish")));
    button.add_event_listener(
        mv!(title, subtitle, content, old_ed => move |_: ClickEvent| {
            title.focus(); // Remove the placeholder before publishing
            set_value("title", title.inner_text());
            subtitle.focus();
            set_value("subtitle", subtitle.inner_text());
            content.focus();
            set_value("editor-content", content.child_nodes().iter().fold(String::new(), |md, ch| {
                let to_append = match ch.node_type() {
                    NodeType::Element => {
                        if js!{ return @{&ch}.tagName; } == "DIV" {
                            (js!{ return @{&ch}.innerHTML; }).try_into().unwrap_or_default()
                        } else {
                            (js!{ return @{&ch}.outerHTML; }).try_into().unwrap_or_default()
                        }
                    },
                    NodeType::Text => ch.node_value().unwrap_or_default(),
                    _ => unreachable!(),
                };
                format!("{}\n\n{}", md, to_append)
            }));
            set_value("tags", get_elt_value("popup-tags"));
            if let Some(draft) = document().get_element_by_id("popup-draft") {
                js!{
                    document.getElementById("draft").checked = @{draft}.checked;
                };
            }
            let cover = document().get_element_by_id("cover").unwrap();
            cover.parent_element().unwrap().remove_child(&cover).ok();
            old_ed.append_child(&cover);
            set_value("license", get_elt_value("popup-license"));
            js! {
                @{&old_ed}.submit();
            };
        }),
    );
    popup.append_child(&button);

    document().body()?.append_child(&popup);
    Ok(popup)
}

fn init_popup_bg() -> Result<Element, EditorError> {
    let bg = document().create_element("div")?;
    bg.class_list().add("popup-bg")?;
    bg.set_attribute("id", "popup-bg")?;

    document().body()?.append_child(&bg);
    bg.add_event_listener(|_: ClickEvent| close_popup());
    Ok(bg)
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

fn close_popup() {
    let hide = |x: Element| x.class_list().remove("show");
    document().get_element_by_id("publish-popup").map(hide);
    document().get_element_by_id("popup-bg").map(hide);
}

fn make_input(label_text: &str, name: &'static str, form: &Element) -> InputElement {
    let label = document().create_element("label").unwrap();
    label.append_child(&document().create_text_node(label_text));
    label.set_attribute("for", name).unwrap();

    let inp: InputElement = document()
        .create_element("input")
        .unwrap()
        .try_into()
        .unwrap();
    inp.set_attribute("name", name).unwrap();
    inp.set_attribute("id", name).unwrap();

    form.append_child(&label);
    form.append_child(&inp);
    inp
}

fn make_editable(tag: &'static str) -> Element {
    let elt = document()
        .create_element(tag)
        .expect("Couldn't create editable element");
    elt.set_attribute("contenteditable", "true")
        .expect("Couldn't make the element editable");
    elt
}

fn clear_children(elt: &HtmlElement) {
    for child in elt.child_nodes() {
        elt.remove_child(&child).unwrap();
    }
}
