use std::rc::Rc;
use stdweb::{unstable::TryInto, web::{*, html_element::*, event::*}};
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
    inp.map(|i| i.raw_value()).unwrap_or_else(|_| textarea.unwrap().value())
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

pub fn init() {
    document().get_element_by_id("plume-editor")
        .and_then(|ed| {
            js!{
                @{&ed}.style.display = "block";
            };

            let title_val = get_elt_value("title");
            let subtitle_val = get_elt_value("subtitle");
            let content_val = get_elt_value("editor-content");

            let old_ed = document().get_element_by_id("plume-fallback-editor")?;
            let old_title = document().get_element_by_id("plume-editor-title")?;
            js! {
                @{&old_ed}.style.display = "none";
                @{&old_title}.style.display = "none";
            };

            let title = placeholder(make_editable("h1").try_into().unwrap(), &i18n!(CATALOG, "Title"));
            if !title_val.is_empty() {
                title.dataset().insert("edited", "true").unwrap();
            }
            title.append_child(&document().create_text_node(&title_val));
            title.add_event_listener(no_return);

            let subtitle = placeholder(make_editable("h2").try_into().unwrap(), &i18n!(CATALOG, "Subtitle or summary"));
            if !subtitle_val.is_empty() {
                subtitle.dataset().insert("edited", "true").unwrap();
            }
            subtitle.append_child(&document().create_text_node(&subtitle_val));
            subtitle.add_event_listener(no_return);

            let content = placeholder(make_editable("article").try_into().unwrap(), &i18n!(CATALOG, "Write your article here. Markdown is supported."));
            if !content_val.is_empty() {
                content.dataset().insert("edited", "true").unwrap();
            }
            content.append_child(&document().create_text_node(&content_val));

            // Add the elements, and make sure the placeholders are rendered
            ed.append_child(&title);
            title.focus();
            title.blur();
            ed.append_child(&subtitle);
            subtitle.focus();
            subtitle.blur();
            ed.append_child(&content);
            content.focus();
            content.blur();

            let widgets = Rc::new((
                title, subtitle, content
            ));
            document().get_element_by_id("publish")?.add_event_listener(mv!(old_ed, widgets => move |_: ClickEvent| {
                document().get_element_by_id("publish-popup")
                    .unwrap_or_else(mv!(old_ed, widgets => move || {
                        let popup = document().create_element("div").unwrap();
                        popup.class_list().add("popup").unwrap();
                        popup.set_attribute("id", "publish-popup").unwrap();

                        let tags = get_elt_value("tags").split(',').map(str::trim).map(str::to_string).collect::<Vec<_>>();
                        let license = get_elt_value("license");
                        make_input(i18n!(CATALOG, "Tags"), "popup-tags", &popup).set_raw_value(&tags.join(", "));
                        make_input(i18n!(CATALOG, "License"), "popup-license", &popup).set_raw_value(&license);

                        let cover_label = document().create_element("label").unwrap();
                        cover_label.append_child(&document().create_text_node(i18n!(CATALOG, "Cover")));
                        cover_label.set_attribute("for", "cover").unwrap();
                        let cover = document().get_element_by_id("cover").unwrap();
                        cover.parent_element().unwrap().remove_child(&cover).ok();
                        popup.append_child(&cover_label);
                        popup.append_child(&cover);

                        let button = document().create_element("input").unwrap();
                        js!{
                            @{&button}.type = "submit";
                            @{&button}.value = @{i18n!(CATALOG, "Publish")};
                        };
                        button.append_child(&document().create_text_node(&i18n!(CATALOG, "Publish")));
                        button.add_event_listener(mv!(widgets, old_ed => move |_: ClickEvent| {
                            set_value("title", widgets.0.inner_text());
                            set_value("subtitle", widgets.1.inner_text());
                            set_value("editor-content", widgets.2.inner_text());
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

                        document().body().unwrap().append_child(&popup);
                        popup
                    })).class_list().add("show").unwrap();

                document().get_element_by_id("popup-bg")
                    .unwrap_or_else(|| {
                        let bg = document().create_element("div").unwrap();
                        bg.class_list().add("popup-bg").unwrap();
                        bg.set_attribute("id", "popup-bg").unwrap();

                        document().body().unwrap().append_child(&bg);
                        bg.add_event_listener(|_: ClickEvent| close_popup());
                        bg
                    }).class_list().add("show").unwrap();
            }));

            Some(())
        });
}

fn close_popup() {
    document().get_element_by_id("publish-popup")
        .map(|popup| popup.class_list().remove("show"));
    document().get_element_by_id("popup-bg")
        .map(|bg| bg.class_list().remove("show"));
}

fn make_input(label_text: &'static str, name: &'static str, form: &Element) -> InputElement {
    let label = document().create_element("label").unwrap();
    label.append_child(&document().create_text_node(label_text));
    label.set_attribute("for", name).unwrap();

    let inp: InputElement = document().create_element("input").unwrap().try_into().unwrap();
    inp.set_attribute("name", name).unwrap();
    inp.set_attribute("id", name).unwrap();

    form.append_child(&label);
    form.append_child(&inp);
    inp
}

fn make_editable(tag: &'static str) -> Element {
    let elt = document().create_element(tag).expect("Couldn't create editable element");
    elt.set_attribute("contenteditable", "true").expect("Couldn't make element editable");
    elt
}

fn placeholder<'a>(elt: HtmlElement, text: &'a str) -> HtmlElement {
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
            ph.append_child(&document().create_text_node(&elt.dataset().get("placeholder").unwrap_or(String::new())));
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
