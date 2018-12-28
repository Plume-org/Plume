use std::{rc::Rc, cell::RefCell};
use stdweb::{unstable::TryInto, web::{*, html_element::*, event::*}};

macro_rules! mv {
    ( $( $var:ident ),* => $exp:expr ) => {
        {
            $( let $var = $var.clone(); )*
            $exp
        }
    }
}

struct Fragment {
    placeholder: Option<String>,
    deletable: bool,
    content: Rc<RefCell<String>>,
    html_tag: &'static str,
    next_tag: Option<&'static str>,
}

impl Fragment {
    fn element(&self) -> HtmlElement {
        let elt: HtmlElement = make_editable(self.html_tag).try_into().unwrap();
        let elt = if let Some(ref ph) = self.placeholder {
            placeholder(elt, ph.as_str())
        } else {
            elt
        };
        if !self.content.borrow().is_empty() {
            elt.dataset().insert("edited", "true").unwrap();
        }
        elt.append_child(&document().create_text_node(self.content.borrow().clone().as_ref()));
        elt
    }

    fn render_to(&mut self, parent: &Element, state: &State) -> HtmlElement {
        let elt = self.element();
        parent.append_child(&elt);
        self.events(&elt, state);
        elt
    }

    fn render_after(&mut self, sibling: &HtmlElement, state: &State) -> HtmlElement {
        let elt = self.element();
        let parent = sibling.parent_element().unwrap();
        parent.replace_child(&elt, sibling).unwrap();
        parent.insert_before(sibling, &elt).unwrap();
        self.events(&elt, state);
        elt
    }

    fn events(&mut self, elt: &HtmlElement, state: &State) {
        // Make sure the placeholder is visible
        elt.focus();
        elt.blur();

        let next_tag = self.next_tag;
        let deletable = self.deletable;
        let cont = &self.content;
        elt.add_event_listener(mv!(state, elt, next_tag, deletable, cont => move |evt: KeyPressEvent| {
            // console!(log, evt.key());
            if evt.key() == "Enter" {
                evt.prevent_default();
                if let Some(next_tag) = next_tag {
                    let mut new = Fragment {
                        placeholder: None,
                        deletable: true,
                        content: Rc::new(RefCell::new(String::new())),
                        html_tag: next_tag,
                        next_tag: Some("p"),
                    };
                    new.render_after(&elt.clone(), &state).focus();
                    let current_index = state.borrow_mut().fragments.iter().position(|x| x.borrow().content == cont).unwrap_or(2);
                    state.borrow_mut().fragments.insert(current_index + 1, Rc::new(RefCell::new(new)));
                } else {
                    let next: HtmlElement = elt.next_sibling().unwrap().try_into().unwrap();
                    next.focus();
                }
            }
            if evt.key() == "Backspace" && elt.inner_text().trim_matches('\n').is_empty() && deletable {
                evt.prevent_default();
                let prev: HtmlElement = elt.previous_sibling().unwrap().try_into().unwrap();
                elt.remove();
                prev.focus();
            }
            if evt.key() == "Delete" && elt.inner_text().trim_matches('\n').is_empty() && deletable {
                evt.prevent_default();
                let next: HtmlElement = elt.next_sibling().unwrap().try_into().unwrap();
                elt.remove();
                next.focus();
            }
            if evt.key() == "ArrowUp" && window().get_selection().unwrap().anchor_offset() == 0 {
                evt.prevent_default();
                let prev: HtmlElement = elt.previous_sibling().unwrap().try_into().unwrap();
                prev.focus();
                window().get_selection().unwrap().select_all_children(&prev);
                window().get_selection().unwrap().collapse_to_end().unwrap();
            }
            if evt.key() == "ArrowDown" /* TODO: && selection at last line */ {
                let next: HtmlElement = elt.next_sibling().unwrap().try_into().unwrap();
                next.focus();
                window().get_selection().unwrap().select_all_children(&next);
                window().get_selection().unwrap().collapse_to_start().unwrap();
            }
        }));
        let content = self.content.clone();
        elt.add_event_listener(mv!(elt => move |_: KeyUpEvent| {
            *content.borrow_mut() = elt.inner_text();
        }));
    }
}

struct Editor {
    csrf: String,
    tags: String,
    cover_id: Option<i32>,
    fragments: Vec<Rc<RefCell<Fragment>>>,
}

impl Editor {
    fn new(title: String, subtitle: String, content: String) -> Self {
        let mut fragments = vec![
            Rc::new(RefCell::new(Fragment {
                placeholder: Some("Title".into()),
                deletable: false,
                content: Rc::new(RefCell::new(title)),
                html_tag: "h1",
                next_tag: None,
            })),
            Rc::new(RefCell::new(Fragment {
                placeholder: Some("Subtitle or summary".into()),
                deletable: false,
                content: Rc::new(RefCell::new(subtitle)),
                html_tag: "h2",
                next_tag: Some("p"),
            })),
        ];
        let mut paragraphs = content.split("\n\n").map(|p| Rc::new(RefCell::new(Fragment {
            placeholder: Some("…".into()),
            deletable: false,
            content: Rc::new(RefCell::new(p.to_string())),
            html_tag: "p",
            next_tag: Some("p"),
        }))).collect();
        fragments.append(&mut paragraphs);
        Editor {
            csrf: String::new(),
            tags: String::new(),
            cover_id: None,
            fragments: fragments
        }
    }
}

type State = Rc<RefCell<Editor>>;

pub fn init() {
    document().get_element_by_id("plume-editor")
        .and_then(|ed| {
            let old_ed = document().get_element_by_id("plume-fallback-editor")?;
            let title: InputElement = document().get_element_by_id("title")?.try_into().ok()?;
            let title = title.raw_value();

            let subtitle: InputElement = document().get_element_by_id("subtitle")?.try_into().ok()?;
            let subtitle = subtitle.raw_value();

            let content: TextAreaElement = document().get_element_by_id("editor-content")?.try_into().ok()?;
            let content = content.value();
            js! {
                @{old_ed}.style.display = "none";
            }

            let state = Rc::new(RefCell::new(Editor::new(title, subtitle, content)));

            let button = document().create_element("button").ok()?;
            button.append_child(&document().create_text_node("print state"));
            button.add_event_listener(mv!(state => move |_: ClickEvent| {
                for f in state.borrow().fragments.clone() {
                    let x = f.borrow();
                    console!(log, x.content.borrow().clone())
                }
            }));
            ed.append_child(&button);

            for frag in state.borrow_mut().fragments.clone() {
                frag.borrow_mut().render_to(&ed, &state);
            }

            document().get_element_by_id("publish")?.add_event_listener(mv!(old_ed, state => move |_: ClickEvent| {
                document().get_element_by_id("publish-popup")
                    .map(|popup| popup.class_list().add("show").unwrap())
                    .unwrap_or_else(|| {
                        let popup = document().create_element("div").unwrap();
                        popup.class_list().add("popup").unwrap();
                        popup.class_list().add("show").unwrap();
                        popup.set_attribute("id", "publish-popup").unwrap();

                        make_input("Tags", "tags", &popup);
                        make_input("License", "license", &popup);

                        document().body().unwrap().append_child(&popup);
                    });
                document().get_element_by_id("popup-bg")
                    .map(|bg| bg.class_list().add("show").unwrap())
                    .unwrap_or_else(|| {
                        let bg = document().create_element("div").unwrap();
                        bg.class_list().add("popup-bg").unwrap();
                        bg.class_list().add("show").unwrap();
                        bg.set_attribute("id", "popup-bg").unwrap();

                        document().body().unwrap().append_child(&bg);
                        bg.add_event_listener(|_: ClickEvent| close_popup());
                    });

                let state = state.borrow();
                let title = state.fragments[0].borrow();
                let title = title.content.borrow().clone();
                let subtitle = state.fragments[1].borrow();
                let subtitle = subtitle.content.borrow().clone();
                let tags = state.tags.clone();
                let cover_id = state.cover_id.clone();
                js! { // URLSearchParams and fetch are not yet available in stdweb
                    let form = new URLSearchParams();
                    form.set("csrf-token", @{csrf});
                    form.set("title", @{title});
                    form.set("subtitle", @{subtitle});
                    form.set("content", @{content});
                    form.set("tags", @{tags});
                    form.set("cover_id", @{cover_id});
                    form.set("license", @{license});

                    fetch(@{action}, {
                        method: "POST",
                        body: form,
                    })
                }
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

fn make_input(label_text: &'static str, name: &'static str, form: &Element) {
    let label = document().create_element("label").unwrap();
    label.append_child(&document().create_text_node(label_text));
    label.set_attribute("for", name).unwrap();

    let inp: InputElement = document().create_element("input").unwrap().try_into().unwrap();
    inp.set_attribute("name", name).unwrap();
    inp.set_attribute("id", name).unwrap();

    form.append_child(&label);
    form.append_child(&inp)
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
