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

fn init_widget(
    parent: &Element,
    tag: &'static str,
    placeholder_text: String,
    content: String,
    disable_return: bool,
) -> Result<HtmlElement, EditorError> {
    let widget = placeholder(make_editable(tag).try_into()?, &placeholder_text);
    if !content.is_empty() {
        widget.dataset().insert("edited", "true")?;
    }
    widget.append_child(&document().create_text_node(&content));
    if disable_return {
        widget.add_event_listener(no_return);
    }

    parent.append_child(&widget);
    // We need to do that to make sure the placeholder is correctly rendered
    widget.focus();
    widget.blur();

    Ok(widget)
}

pub fn init() -> Result<(), EditorError> {
    if let Some(ed) = document().get_element_by_id("plume-editor") {
        // Show the editor
        js! { @{&ed}.style.display = "block"; };
        // And hide the HTML-only fallback
        let old_ed = document().get_element_by_id("plume-fallback-editor")?;
        let old_title = document().get_element_by_id("plume-editor-title")?;
        js! {
            @{&old_ed}.style.display = "none";
            @{&old_title}.style.display = "none";
        };

        // Get content from the old editor (when editing an article for instance)
        let title_val = get_elt_value("title");
        let subtitle_val = get_elt_value("subtitle");
        let content_val = get_elt_value("editor-content");
        // And pre-fill the new editor with this values
        let title = init_widget(&ed, "h1", i18n!(CATALOG, "Title"), title_val, true)?;
        let subtitle = init_widget(
            &ed,
            "h2",
            i18n!(CATALOG, "Subtitle or summary"),
            subtitle_val,
            true,
        )?;
        let content = init_widget(
            &ed,
            "article",
            i18n!(CATALOG, "Write your article here. Markdown is supported."),
            content_val.clone(),
            true,
        )?;
        js! { @{&content}.innerHTML = @{content_val}; };

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

        document().get_element_by_id("publish")?.add_event_listener(
            mv!(title, subtitle, content, old_ed => move |_: ClickEvent| {
                let popup = document().get_element_by_id("publish-popup").or_else(||
                        init_popup(&title, &subtitle, &content, &old_ed).ok()
                    ).unwrap();
                let bg = document().get_element_by_id("popup-bg").or_else(||
                        init_popup_bg().ok()
                    ).unwrap();

                popup.class_list().add("show").unwrap();
                bg.class_list().add("show").unwrap();
            }),
        );
    }
    Ok(())
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

    let button = document().create_element("input")?;
    js! {
        @{&button}.type = "submit";
        @{&button}.value = @{i18n!(CATALOG, "Publish")};
    };
    button.append_child(&document().create_text_node(&i18n!(CATALOG, "Publish")));
    button.add_event_listener(mv!(title, subtitle, content, old_ed => move |_: ClickEvent| {
        set_value("title", title.inner_text());
        set_value("subtitle", subtitle.inner_text());
        set_value("editor-content", js!{ return @{&content}.innerHTML }.as_str().unwrap_or_default());
        set_value("tags", get_elt_value("popup-tags"));
        let cover = document().get_element_by_id("cover").unwrap();
        cover.parent_element().unwrap().remove_child(&cover).ok();
        old_ed.append_child(&cover);
        set_value("license", get_elt_value("popup-license"));
        js! {
            @{&old_ed}.submit();
        };
    }));
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

fn chars_left(selector: &str, content: &HtmlElement) -> Option<i32> {
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
        .expect("Couldn't make element editable");
    elt
}

fn placeholder(elt: HtmlElement, text: &str) -> HtmlElement {
    elt.dataset().insert("placeholder", text).unwrap();
    elt.dataset().insert("edited", "false").unwrap();

    elt.add_event_listener(mv!(elt => move |_: FocusEvent| {
        if elt.dataset().get("edited").unwrap().as_str() != "true" {
            clear_children(&elt);
        }
    }));
    elt.add_event_listener(mv!(elt => move |_: BlurEvent| {
        if elt.dataset().get("edited").unwrap().as_str() != "true" {
            clear_children(&elt);

            let ph = document().create_element("span").expect("Couldn't create placeholder");
            ph.class_list().add("placeholder").expect("Couldn't add class");
            ph.append_child(&document().create_text_node(&elt.dataset().get("placeholder").unwrap_or_default()));
            elt.append_child(&ph);
        }
    }));
    elt.add_event_listener(mv!(elt => move |_: KeyUpEvent| {
        elt.dataset().insert("edited", if elt.inner_text().trim_matches('\n').is_empty() {
            "false"
        } else {
            "true"
        }).expect("Couldn't update edition state");
    }));
    elt
}

fn clear_children(elt: &HtmlElement) {
    for child in elt.child_nodes() {
        elt.remove_child(&child).unwrap();
    }
}
