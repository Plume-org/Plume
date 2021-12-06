use crate::{document, CATALOG};
use js_sys::{encode_uri_component, Date, RegExp};
use serde_derive::{Deserialize, Serialize};
use std::{convert::TryInto, sync::Mutex};
use wasm_bindgen::{prelude::*, JsCast, JsValue};
use web_sys::{
    console, window, ClipboardEvent, Element, Event, FocusEvent, HtmlAnchorElement, HtmlDocument,
    HtmlElement, HtmlFormElement, HtmlInputElement, HtmlSelectElement, HtmlTextAreaElement,
    KeyboardEvent, MouseEvent, Node,
};

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
    let inp: Option<&HtmlInputElement> = elt.dyn_ref();
    let textarea: Option<&HtmlTextAreaElement> = elt.dyn_ref();
    let select: Option<&HtmlSelectElement> = elt.dyn_ref();
    inp.map(|i| i.value()).unwrap_or_else(|| {
        textarea
            .map(|t| t.value())
            .unwrap_or_else(|| select.unwrap().value())
    })
}

fn set_value<S: AsRef<str>>(id: &'static str, val: S) {
    let elt = document().get_element_by_id(id).unwrap();
    let inp: Option<&HtmlInputElement> = elt.dyn_ref();
    let textarea: Option<&HtmlTextAreaElement> = elt.dyn_ref();
    let select: Option<&HtmlSelectElement> = elt.dyn_ref();
    inp.map(|i| i.set_value(val.as_ref())).unwrap_or_else(|| {
        textarea
            .map(|t| t.set_value(val.as_ref()))
            .unwrap_or_else(|| select.unwrap().set_value(val.as_ref()))
    })
}

fn no_return(evt: KeyboardEvent) {
    if evt.key() == "Enter" {
        evt.prevent_default();
    }
}

#[derive(Debug)]
pub enum EditorError {
    NoneError,
    DOMError,
}

const AUTOSAVE_DEBOUNCE_TIME: i32 = 5000;
#[derive(Serialize, Deserialize)]
struct AutosaveInformation {
    contents: String,
    cover: String,
    last_saved: f64,
    license: String,
    subtitle: String,
    tags: String,
    title: String,
}
fn is_basic_editor() -> bool {
    if let Some(basic_editor) = window()
        .unwrap()
        .local_storage()
        .unwrap()
        .unwrap()
        .get("basic-editor")
        .unwrap()
    {
        &basic_editor == "true"
    } else {
        false
    }
}
fn get_title() -> String {
    if is_basic_editor() {
        get_elt_value("title")
    } else {
        document()
            .query_selector("#plume-editor > h1")
            .unwrap()
            .unwrap()
            .dyn_ref::<HtmlElement>()
            .unwrap()
            .inner_text()
    }
}
fn get_autosave_id() -> String {
    format!(
        "editor_contents={}",
        window().unwrap().location().pathname().unwrap()
    )
}
fn get_editor_contents() -> String {
    if is_basic_editor() {
        get_elt_value("editor-content")
    } else {
        let editor = document().query_selector("article").unwrap().unwrap();
        let child_nodes = editor.child_nodes();
        let mut md = String::new();
        for i in 0..child_nodes.length() {
            let ch = child_nodes.get(i).unwrap();
            let to_append = match ch.node_type() {
                Node::ELEMENT_NODE => {
                    let elt = ch.dyn_ref::<Element>().unwrap();
                    if elt.tag_name() == "DIV" {
                        elt.inner_html()
                    } else {
                        elt.outer_html()
                    }
                }
                Node::TEXT_NODE => ch.node_value().unwrap_or_default(),
                _ => unreachable!(),
            };
            md = format!("{}\n\n{}", md, to_append);
        }
        md
    }
}
fn get_subtitle() -> String {
    if is_basic_editor() {
        get_elt_value("subtitle")
    } else {
        document()
            .query_selector("#plume-editor > h2")
            .unwrap()
            .unwrap()
            .dyn_ref::<HtmlElement>()
            .unwrap()
            .inner_text()
    }
}
fn autosave() {
    let info = AutosaveInformation {
        contents: get_editor_contents(),
        title: get_title(),
        subtitle: get_subtitle(),
        tags: get_elt_value("tags"),
        license: get_elt_value("license"),
        last_saved: Date::now(),
        cover: get_elt_value("cover"),
    };
    let id = get_autosave_id();
    match window()
        .unwrap()
        .local_storage()
        .unwrap()
        .unwrap()
        .set(&id, &serde_json::to_string(&info).unwrap())
    {
        Ok(_) => {}
        _ => console::log_1(&"Autosave failed D:".into()),
    }
}
fn load_autosave() {
    if let Ok(Some(autosave_str)) = window()
        .unwrap()
        .local_storage()
        .unwrap()
        .unwrap()
        .get(&get_autosave_id())
    {
        let autosave_info: AutosaveInformation = serde_json::from_str(&autosave_str).ok().unwrap();
        let message = i18n!(
            CATALOG,
            "Do you want to load the local autosave last edited at {}?";
            Date::new(&JsValue::from_f64(autosave_info.last_saved)).to_date_string().as_string().unwrap()
        );
        if let Ok(true) = window().unwrap().confirm_with_message(&message) {
            set_value("editor-content", &autosave_info.contents);
            set_value("title", &autosave_info.title);
            set_value("subtitle", &autosave_info.subtitle);
            set_value("tags", &autosave_info.tags);
            set_value("license", &autosave_info.license);
            set_value("cover", &autosave_info.cover);
        } else {
            clear_autosave();
        }
    }
}
fn clear_autosave() {
    window()
        .unwrap()
        .local_storage()
        .unwrap()
        .unwrap()
        .remove_item(&get_autosave_id())
        .unwrap();
    console::log_1(&format!("Saved to {}", &get_autosave_id()).into());
}
type TimeoutHandle = i32;
lazy_static! {
    static ref AUTOSAVE_TIMEOUT: Mutex<Option<TimeoutHandle>> = Mutex::new(None);
}
fn autosave_debounce() {
    let window = window().unwrap();
    let timeout = &mut AUTOSAVE_TIMEOUT.lock().unwrap();
    if let Some(timeout) = timeout.take() {
        window.clear_timeout_with_handle(timeout);
    }
    let callback = Closure::once(autosave);
    **timeout = window
        .set_timeout_with_callback_and_timeout_and_arguments_0(
            callback.as_ref().unchecked_ref(),
            AUTOSAVE_DEBOUNCE_TIME,
        )
        .ok();
    callback.forget();
}
fn init_widget(
    parent: &Element,
    tag: &'static str,
    placeholder_text: String,
    content: String,
    disable_return: bool,
) -> Result<HtmlElement, EditorError> {
    let widget = placeholder(
        make_editable(tag).dyn_into::<HtmlElement>().unwrap(),
        &placeholder_text,
    );
    if !content.is_empty() {
        widget
            .dataset()
            .set("edited", "true")
            .map_err(|_| EditorError::DOMError)?;
    }
    widget
        .append_child(&document().create_text_node(&content))
        .map_err(|_| EditorError::DOMError)?;
    if disable_return {
        let callback = Closure::wrap(Box::new(no_return) as Box<dyn FnMut(KeyboardEvent)>);
        widget
            .add_event_listener_with_callback("keydown", callback.as_ref().unchecked_ref())
            .map_err(|_| EditorError::DOMError)?;
        callback.forget();
    }

    parent
        .append_child(&widget)
        .map_err(|_| EditorError::DOMError)?;
    // We need to do that to make sure the placeholder is correctly rendered
    widget.focus().map_err(|_| EditorError::DOMError)?;
    widget.blur().map_err(|_| EditorError::DOMError)?;

    filter_paste(&widget);

    Ok(widget)
}

fn filter_paste(elt: &HtmlElement) {
    // Only insert text when pasting something
    let insert_text = Closure::wrap(Box::new(|evt: ClipboardEvent| {
        evt.prevent_default();
        if let Some(data) = evt.clipboard_data() {
            if let Ok(data) = data.get_data("text") {
                document()
                    .dyn_ref::<HtmlDocument>()
                    .unwrap()
                    .exec_command_with_show_ui_and_value("insertText", false, &data)
                    .unwrap();
            }
        }
    }) as Box<dyn FnMut(ClipboardEvent)>);
    elt.add_event_listener_with_callback("paste", insert_text.as_ref().unchecked_ref())
        .unwrap();
    insert_text.forget();
}

pub fn init() -> Result<(), EditorError> {
    if let Some(ed) = document().get_element_by_id("plume-fallback-editor") {
        load_autosave();
        let callback = Closure::wrap(Box::new(|_| clear_autosave()) as Box<dyn FnMut(Event)>);
        ed.add_event_listener_with_callback("submit", callback.as_ref().unchecked_ref())
            .map_err(|_| EditorError::DOMError)?;
        callback.forget();
    }
    // Check if the user wants to use the basic editor
    if window()
        .unwrap()
        .local_storage()
        .unwrap()
        .unwrap()
        .get("basic-editor")
        .map(|x| x.is_some() && x.unwrap() == "true")
        .unwrap_or(true)
    {
        if let Some(editor) = document().get_element_by_id("plume-fallback-editor") {
            if let Ok(Some(title_label)) = document().query_selector("label[for=title]") {
                let editor_button = document()
                    .create_element("a")
                    .map_err(|_| EditorError::DOMError)?;
                editor_button
                    .dyn_ref::<HtmlAnchorElement>()
                    .unwrap()
                    .set_href("#");
                let disable_basic_editor = Closure::wrap(Box::new(|_| {
                    let window = window().unwrap();
                    if window
                        .local_storage()
                        .unwrap()
                        .unwrap()
                        .set("basic-editor", "false")
                        .is_err()
                    {
                        console::log_1(&"Failed to write into local storage".into());
                    }
                    window.history().unwrap().go_with_delta(0).ok(); // refresh
                })
                    as Box<dyn FnMut(MouseEvent)>);
                editor_button
                    .add_event_listener_with_callback(
                        "click",
                        disable_basic_editor.as_ref().unchecked_ref(),
                    )
                    .map_err(|_| EditorError::DOMError)?;
                disable_basic_editor.forget();
                editor_button
                    .append_child(
                        &document().create_text_node(&i18n!(CATALOG, "Open the rich text editor")),
                    )
                    .map_err(|_| EditorError::DOMError)?;
                editor
                    .insert_before(&editor_button, Some(&title_label))
                    .ok();
                let callback = Closure::wrap(
                    Box::new(|_| autosave_debounce()) as Box<dyn FnMut(KeyboardEvent)>
                );
                document()
                    .get_element_by_id("editor-content")
                    .unwrap()
                    .add_event_listener_with_callback("keydown", callback.as_ref().unchecked_ref())
                    .map_err(|_| EditorError::DOMError)?;
                callback.forget();
            }
        }

        Ok(())
    } else {
        init_editor()
    }
}

fn init_editor() -> Result<(), EditorError> {
    if let Some(ed) = document().get_element_by_id("plume-editor") {
        // Show the editor
        ed.dyn_ref::<HtmlElement>()
            .unwrap()
            .style()
            .set_property("display", "block")
            .map_err(|_| EditorError::DOMError)?;
        // And hide the HTML-only fallback
        let old_ed = document().get_element_by_id("plume-fallback-editor");
        if old_ed.is_none() {
            return Ok(());
        }
        let old_ed = old_ed.unwrap();
        let old_title = document()
            .get_element_by_id("plume-editor-title")
            .ok_or(EditorError::NoneError)?;
        old_ed
            .dyn_ref::<HtmlElement>()
            .unwrap()
            .style()
            .set_property("display", "none")
            .map_err(|_| EditorError::DOMError)?;
        old_title
            .dyn_ref::<HtmlElement>()
            .unwrap()
            .style()
            .set_property("display", "none")
            .map_err(|_| EditorError::DOMError)?;

        // Get content from the old editor (when editing an article for instance)
        let title_val = get_elt_value("title");
        let subtitle_val = get_elt_value("subtitle");
        let content_val = get_elt_value("editor-content");
        // And pre-fill the new editor with this values
        let title = init_widget(&ed, "h1", i18n!(CATALOG, "Title"), title_val, true)?;
        let subtitle = init_widget(
            &ed,
            "h2",
            i18n!(CATALOG, "Subtitle, or summary"),
            subtitle_val,
            true,
        )?;
        let content = init_widget(
            &ed,
            "article",
            i18n!(CATALOG, "Write your article here. Markdown is supported."),
            content_val.clone(),
            false,
        )?;
        content.set_inner_html(&content_val);

        // character counter
        let character_counter = Closure::wrap(Box::new(mv!(content => move |_| {
            let update_char_count = Closure::wrap(Box::new(mv!(content => move || {
                if let Some(e) = document().get_element_by_id("char-count") {
                    let count = chars_left("#plume-fallback-editor", &content).unwrap_or_default();
                    let text = i18n!(CATALOG, "Around {} characters left"; count);
                    e.dyn_ref::<HtmlElement>().map(|e| {
                        e.set_inner_text(&text);
                    }).unwrap();
                };
            })) as Box<dyn FnMut()>);
            window().unwrap().set_timeout_with_callback_and_timeout_and_arguments(update_char_count.as_ref().unchecked_ref(), 0, &js_sys::Array::new()).unwrap();
            update_char_count.forget();
            autosave_debounce();
        })) as Box<dyn FnMut(KeyboardEvent)>);
        content
            .add_event_listener_with_callback("keydown", character_counter.as_ref().unchecked_ref())
            .map_err(|_| EditorError::DOMError)?;
        character_counter.forget();

        let show_popup = Closure::wrap(Box::new(mv!(title, subtitle, content, old_ed => move |_| {
            let popup = document().get_element_by_id("publish-popup").or_else(||
                                                                              init_popup(&title, &subtitle, &content, &old_ed).ok()
            ).unwrap();
            let bg = document().get_element_by_id("popup-bg").or_else(||
                                                                      init_popup_bg().ok()
            ).unwrap();

            popup.class_list().add_1("show").unwrap();
            bg.class_list().add_1("show").unwrap();
        })) as Box<dyn FnMut(MouseEvent)>);
        document()
            .get_element_by_id("publish")
            .ok_or(EditorError::NoneError)?
            .add_event_listener_with_callback("click", show_popup.as_ref().unchecked_ref())
            .map_err(|_| EditorError::DOMError)?;
        show_popup.forget();

        show_errors();
        setup_close_button();
    }
    Ok(())
}

fn setup_close_button() {
    if let Some(button) = document().get_element_by_id("close-editor") {
        let close_editor = Closure::wrap(Box::new(|_| {
            window()
                .unwrap()
                .local_storage()
                .unwrap()
                .unwrap()
                .set("basic-editor", "true")
                .unwrap();
            window()
                .unwrap()
                .history()
                .unwrap()
                .go_with_delta(0)
                .unwrap(); // Refresh the page
        }) as Box<dyn FnMut(MouseEvent)>);
        button
            .add_event_listener_with_callback("click", close_editor.as_ref().unchecked_ref())
            .unwrap();
        close_editor.forget();
    }
}

fn show_errors() {
    let document = document();
    if let Ok(Some(header)) = document.query_selector("header") {
        let list = document.create_element("header").unwrap();
        list.class_list().add_1("messages").unwrap();
        let errors = document.query_selector_all("p.error").unwrap();
        for i in 0..errors.length() {
            let error = errors.get(i).unwrap();
            error
                .parent_element()
                .unwrap()
                .remove_child(&error)
                .unwrap();
            let _ = list.append_child(&error);
        }
        header
            .parent_element()
            .unwrap()
            .insert_before(&list, header.next_sibling().as_ref())
            .unwrap();
    }
}

fn init_popup(
    title: &HtmlElement,
    subtitle: &HtmlElement,
    content: &HtmlElement,
    old_ed: &Element,
) -> Result<Element, EditorError> {
    let document = document();
    let popup = document
        .create_element("div")
        .map_err(|_| EditorError::DOMError)?;
    popup
        .class_list()
        .add_1("popup")
        .map_err(|_| EditorError::DOMError)?;
    popup
        .set_attribute("id", "publish-popup")
        .map_err(|_| EditorError::DOMError)?;

    let tags = get_elt_value("tags")
        .split(',')
        .map(str::trim)
        .map(str::to_string)
        .collect::<Vec<_>>();
    let license = get_elt_value("license");
    make_input(&i18n!(CATALOG, "Tags"), "popup-tags", &popup).set_value(&tags.join(", "));
    make_input(&i18n!(CATALOG, "License"), "popup-license", &popup).set_value(&license);

    let cover_label = document
        .create_element("label")
        .map_err(|_| EditorError::DOMError)?;
    cover_label
        .append_child(&document.create_text_node(&i18n!(CATALOG, "Cover")))
        .map_err(|_| EditorError::DOMError)?;
    cover_label
        .set_attribute("for", "cover")
        .map_err(|_| EditorError::DOMError)?;
    let cover = document
        .get_element_by_id("cover")
        .ok_or(EditorError::NoneError)?;
    cover
        .parent_element()
        .ok_or(EditorError::NoneError)?
        .remove_child(&cover)
        .ok();
    popup
        .append_child(&cover_label)
        .map_err(|_| EditorError::DOMError)?;
    popup
        .append_child(&cover)
        .map_err(|_| EditorError::DOMError)?;

    if let Some(draft_checkbox) = document.get_element_by_id("draft") {
        let draft_checkbox = draft_checkbox.dyn_ref::<HtmlInputElement>().unwrap();
        let draft_label = document
            .create_element("label")
            .map_err(|_| EditorError::DOMError)?;
        draft_label
            .set_attribute("for", "popup-draft")
            .map_err(|_| EditorError::DOMError)?;

        let draft = document.create_element("input").unwrap();
        draft.set_id("popup-draft");
        let draft = draft.dyn_ref::<HtmlInputElement>().unwrap();
        draft.set_name("popup-draft");
        draft.set_type("checkbox");
        draft.set_checked(draft_checkbox.checked());

        draft_label
            .append_child(draft)
            .map_err(|_| EditorError::DOMError)?;
        draft_label
            .append_child(&document.create_text_node(&i18n!(CATALOG, "This is a draft")))
            .map_err(|_| EditorError::DOMError)?;
        popup
            .append_child(&draft_label)
            .map_err(|_| EditorError::DOMError)?;
    }

    let button = document
        .create_element("input")
        .map_err(|_| EditorError::DOMError)?;
    button
        .append_child(&document.create_text_node(&i18n!(CATALOG, "Publish")))
        .map_err(|_| EditorError::DOMError)?;
    let button = button.dyn_ref::<HtmlInputElement>().unwrap();
    button.set_type("submit");
    button.set_value(&i18n!(CATALOG, "Publish"));
    let callback = Closure::wrap(Box::new(mv!(title, subtitle, content, old_ed => move |_| {
        let document = self::document();
        title.focus().unwrap(); // Remove the placeholder before publishing
        set_value("title", title.inner_text());
        subtitle.focus().unwrap();
        set_value("subtitle", subtitle.inner_text());
        content.focus().unwrap();
        let mut md = String::new();
        let child_nodes = content.child_nodes();
        for i in 0..child_nodes.length() {
            let ch = child_nodes.get(i).unwrap();
            let to_append = match ch.node_type() {
                Node::ELEMENT_NODE => {
                    let ch = ch.dyn_ref::<Element>().unwrap();
                    if ch.tag_name() == "DIV" {
                        ch.inner_html()
                    } else {
                        ch.outer_html()
                    }
                },
                Node::TEXT_NODE => ch.node_value().unwrap_or_default(),
                _ => unreachable!(),
            };
            md = format!("{}\n\n{}", md, to_append);
        }
        set_value("editor-content", md);
        set_value("tags", get_elt_value("popup-tags"));
        if let Some(draft) = document.get_element_by_id("popup-draft") {
            if let Some(draft_checkbox) = document.get_element_by_id("draft") {
                let draft_checkbox = draft_checkbox.dyn_ref::<HtmlInputElement>().unwrap();
                let draft = draft.dyn_ref::<HtmlInputElement>().unwrap();
                draft_checkbox.set_checked(draft.checked());
            }
        }
        let cover = document.get_element_by_id("cover").unwrap();
        cover.parent_element().unwrap().remove_child(&cover).ok();
        old_ed.append_child(&cover).unwrap();
        set_value("license", get_elt_value("popup-license"));
        clear_autosave();
        let old_ed = old_ed.dyn_ref::<HtmlFormElement>().unwrap();
        old_ed.submit().unwrap();
    })) as Box<dyn FnMut(MouseEvent)>);
    button
        .add_event_listener_with_callback("click", callback.as_ref().unchecked_ref())
        .map_err(|_| EditorError::DOMError)?;
    callback.forget();
    popup
        .append_child(button)
        .map_err(|_| EditorError::DOMError)?;

    document
        .body()
        .ok_or(EditorError::NoneError)?
        .append_child(&popup)
        .map_err(|_| EditorError::DOMError)?;
    Ok(popup)
}

fn init_popup_bg() -> Result<Element, EditorError> {
    let bg = document()
        .create_element("div")
        .map_err(|_| EditorError::DOMError)?;
    bg.class_list()
        .add_1("popup-bg")
        .map_err(|_| EditorError::DOMError)?;
    bg.set_attribute("id", "popup-bg")
        .map_err(|_| EditorError::DOMError)?;

    document()
        .body()
        .ok_or(EditorError::NoneError)?
        .append_child(&bg)
        .map_err(|_| EditorError::DOMError)?;
    let callback = Closure::wrap(Box::new(|_| close_popup()) as Box<dyn FnMut(MouseEvent)>);
    bg.add_event_listener_with_callback("click", callback.as_ref().unchecked_ref())
        .unwrap();
    callback.forget();
    Ok(bg)
}

fn chars_left(selector: &str, content: &HtmlElement) -> Option<i32> {
    match document().query_selector(selector) {
        Ok(Some(form)) => form.dyn_ref::<HtmlElement>().and_then(|form| {
            if let Some(len) = form
                .get_attribute("content-size")
                .and_then(|s| s.parse::<i32>().ok())
            {
                (encode_uri_component(&content.inner_html())
                    .replace("%20", "+")
                    .replace("%0A", "%0D0A")
                    .replace_by_pattern(&RegExp::new("[!'*()]", "g"), "XXX")
                    .length()
                    + 2_u32)
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
    let hide = |x: Element| x.class_list().remove_1("show");
    document().get_element_by_id("publish-popup").map(hide);
    document().get_element_by_id("popup-bg").map(hide);
}

fn make_input(label_text: &str, name: &'static str, form: &Element) -> HtmlInputElement {
    let document = document();
    let label = document.create_element("label").unwrap();
    label
        .append_child(&document.create_text_node(label_text))
        .unwrap();
    label.set_attribute("for", name).unwrap();

    let inp = document.create_element("input").unwrap();
    let inp = inp.dyn_into::<HtmlInputElement>().unwrap();
    inp.set_attribute("name", name).unwrap();
    inp.set_attribute("id", name).unwrap();

    form.append_child(&label).unwrap();
    form.append_child(&inp).unwrap();
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

fn placeholder(elt: HtmlElement, text: &str) -> HtmlElement {
    elt.dataset().set("placeholder", text).unwrap();
    elt.dataset().set("edited", "false").unwrap();

    let callback = Closure::wrap(Box::new(mv!(elt => move |_: FocusEvent| {
        if elt.dataset().get("edited").unwrap().as_str() != "true" {
            clear_children(&elt);
        }
    })) as Box<dyn FnMut(FocusEvent)>);
    elt.add_event_listener_with_callback("focus", callback.as_ref().unchecked_ref())
        .unwrap();
    callback.forget();
    let callback = Closure::wrap(Box::new(mv!(elt => move |_: Event| {
        if elt.dataset().get("edited").unwrap().as_str() != "true" {
            clear_children(&elt);

            let ph = document().create_element("span").expect("Couldn't create placeholder");
            ph.class_list().add_1("placeholder").expect("Couldn't add class");
            ph.append_child(&document().create_text_node(&elt.dataset().get("placeholder").unwrap_or_default())).unwrap();
            elt.append_child(&ph).unwrap();
        }
    })) as Box<dyn FnMut(Event)>);
    elt.add_event_listener_with_callback("blur", callback.as_ref().unchecked_ref())
        .unwrap();
    callback.forget();
    let callback = Closure::wrap(Box::new(mv!(elt => move |_: KeyboardEvent| {
        elt.dataset().set("edited", if elt.inner_text().trim_matches('\n').is_empty() {
            "false"
        } else {
            "true"
        }).expect("Couldn't update edition state");
    })) as Box<dyn FnMut(KeyboardEvent)>);
    elt.add_event_listener_with_callback("keyup", callback.as_ref().unchecked_ref())
        .unwrap();
    callback.forget();
    elt
}

fn clear_children(elt: &HtmlElement) {
    let child_nodes = elt.child_nodes();
    for _ in 0..child_nodes.length() {
        elt.remove_child(&child_nodes.get(0).unwrap()).unwrap();
    }
}
