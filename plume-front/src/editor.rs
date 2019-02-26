/*use std::fmt::Debug;

pub fn init() {
    let text_area = document().create_element("textarea").unwrap();
    let mut editor = Editor::new();
    document().body().unwrap().append_child(&editor.element);
    text_area.add_event_listener(move |evt: KeyUpEvent| {
        let node: TextAreaElement = evt.target().unwrap().try_into().unwrap();
        editor.write(node.value());
        node.set_value("");
        editor.render_all();
        console!(log, format!("{:#?}", editor));
    });
    document().body().unwrap().append_child(&text_area);
}

#[derive(Debug, Default)]
struct Selection {
    start_node: usize,
    start_offset: usize,
    end_node: usize,
    end_offset: usize,
}

impl Selection {
    fn is_caret(&self) -> bool {
        self.start_node == self.end_node && self.start_offset == self.end_offset
    }
}

#[derive(Debug)]
struct Editor {
    selection: Selection,
    /// Ordered list of nodes (titles, paragraphs, images, etc)
    nodes: Vec<Rc<RefCell<dyn Node>>>,
    element: Element,
}

impl Editor {
    fn new() -> Editor {
        Editor {
            selection: Selection::default(),
            nodes: vec![Rc::new(RefCell::new(Paragraph { text: String::new() }))],
            element: Self::make_root()
        }
    }

    fn make_root() -> Element {
        let elt = document().create_element("div").unwrap();
        elt
    }

    fn render_node(&self, node_id: usize) {
        let node = *self.nodes[node_id].borrow();
        let elt = node.render();

        if self.selection.is_caret() {
            if self.selection.start_node == node_id {
                utils::insert_html(&elt, self.selection.start_offset, "<span class=\"caret\"></span>");
            }
        } else {
            let sel_start = if self.selection.start_node == node_id {
                Some(self.selection.start_offset)
            } else if self.selection.start_node < node {
                Some(0)
            } else {
                None
            };

            let sel_end = if self.selection.end_node == node_id {
                Some(self.selection.end_offset)
            } else if self.selection.end_node > node {
                Some(utils::elt_len(elt))
            } else {
                None
            };

            if sel_start.is_some() && sel_end.is_some() {
                let sel = document().create_element("span");
                sel.class_list().add("selected");
                utils::wrap_with(&elt, sel_start.unwrap(), sel_end.unwrap(), &sel);
            }
        }

        let id = format!("plume-editor-{}", node_id);
        if let Some(old_node) = document().get_element_by_id(&id) {
            self.element.replace_child(&elt, &old_node);
        } else {
            self.element.append_child(&elt);
        }
    }

    fn select(&mut self, new_selection: Selection) {
        let old_sel = self.selection;
        self.selection = new_selection;

        // re-render de-selected nodes…
        for id in old_sel.start_node..old_sel.end_node {
            self.render_node(id);
        }
        // and the newly selected ones
        for id in self.selection.start_node..self.selection.end_node {
            self.render_node(id);
        }
    }

    fn render_all(&self) {
        for child in self.element.child_nodes() {
            self.element.remove_child(&child).unwrap();
        }
        for node in self.nodes.clone() {
            self.element.append_child(&node.borrow().render());
        }
    }

    fn write(&mut self, text: String) {
        let node = self.nodes.iter().next().unwrap();
        node.borrow_mut().write(text);
    }
}

trait Node: Debug {
    fn render(&self) -> Element;
    fn write(&mut self, text: String);
}

#[derive(Debug)]
struct Paragraph {
    text: String,
}

impl Node for Paragraph {
    fn render(&self) -> Element {
        let elt = document().create_element("p").unwrap();
        elt.append_child(&document().create_text_node(&self.text));
        elt
    }

    fn write(&mut self, text: String) {
        self.text += &text;
    }
}

mod utils {
    fn wrap_with(elt: &Element, start: usize, end: usize, html: &'static str) {
        let mut current_offset = 0;
        let children = elt.child_nodes();
        loop {
            if let Some(ch) = children.next() {
                let diff = offset - current_offset;
                current_offset += elt_len(ch);
                if current_offset >= offset {
                    let after = js!{
                        return @{&ch}.splitAt(@{diff});
                    };
                    elt.insert_before()
                }
            } else {
                break;
            }
        }
    }

    fn elt_len<T: INode>(elt: &T) -> usize {
        elt.child_nodes().reduce(0, |total, ch| {
            if let Ok(text): TextNode = ch.try_into() {
                total += text.
            }
        })
    }
}



*/




















































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

/*struct Fragment {
    placeholder: Option<String>,
    deletable: bool,
    content: Rc<RefCell<String>>,
    html_tag: &'static str,
    next_tag: Option<&'static str>,
}

/// Utils to have usefull errors in the console
///
/// By default only "unreachable executed" is logged
fn opt<T>(opt: Option<T>, msg: &'static str) -> T {
    match opt {
        Some(t) => t,
        None => {
            console!(log, format!("{}: value was None", msg));
            panic!("");
        }
    }
}

fn unres<T, E: std::fmt::Debug>(opt: Result<T, E>, msg: &'static str) -> T {
    match opt {
        Ok(t) => t,
        Err(e) => {
            console!(log, format!("{}: Error: {:?}", msg, e));
            panic!("");
        }
    }
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
        elt.append_child(&document().create_element("br").unwrap());
        window().get_selection().unwrap().set_base_and_extent(&elt, 0, &elt, 0).ok();
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
        elt.add_event_listener(mv!(state, elt, next_tag, deletable, cont => move |evt: KeyDownEvent| {
            console!(log, evt.key());
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
                window().get_selection().unwrap().select_all_children(&prev);
                window().get_selection().unwrap().collapse_to_end().unwrap();
            }
            if evt.key() == "Delete" && elt.inner_text().trim_matches('\n').is_empty() && deletable {
                evt.prevent_default();
                let next: HtmlElement = elt.next_sibling().unwrap().try_into().unwrap();
                elt.remove();
                next.focus();
            }
            let empty = if elt.inner_text().len() == 0 { 1 } else { 0 };
            if evt.key() == "ArrowUp" && window().get_selection().unwrap().anchor_offset() == empty {
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
    cover_id: Option<i32>,
    fragments: Vec<Rc<RefCell<Fragment>>>,
    license: String,
    tags: Vec<String>,
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
            tags: vec![],
            cover_id: None,
            fragments: fragments,
            license: String::from("CC-BY-SA"),
        }
    }
}*/

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

// type State = Rc<RefCell<Editor>>;

fn no_return(evt: KeyDownEvent) {
    if evt.key() == "Enter" {
        evt.prevent_default();
    }
}

pub fn init() {
    document().get_element_by_id("plume-editor")
        .and_then(|ed| {
            let title_val = get_elt_value("title");
            let subtitle_val = get_elt_value("subtitle");
            let content_val = get_elt_value("editor-content");

            let old_ed = document().get_element_by_id("plume-fallback-editor")?;
            let old_title = document().get_element_by_id("plume-editor-title")?;
            js! {
                @{&old_ed}.style.display = "none";
                @{&old_title}.style.display = "none";
            };

            let title = placeholder(make_editable("h1").try_into().unwrap(), "Title");
            if !title_val.is_empty() {
                title.dataset().insert("edited", "true").unwrap();
            }
            title.append_child(&document().create_text_node(&title_val));
            title.add_event_listener(no_return);

            let subtitle = placeholder(make_editable("h2").try_into().unwrap(), "Subtitle or summary");
            if !subtitle_val.is_empty() {
                subtitle.dataset().insert("edited", "true").unwrap();
            }
            subtitle.append_child(&document().create_text_node(&subtitle_val));
            subtitle.add_event_listener(no_return);

            let content = placeholder(make_editable("article").try_into().unwrap(), "Write your article here. Markdown is supported.");
            if !content_val.is_empty() {
                content.dataset().insert("edited", "true").unwrap();
            }
            content.append_child(&document().create_text_node(&content_val));

            ed.append_child(&title);
            ed.append_child(&subtitle);
            ed.append_child(&content);

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
                        make_input("Tags", "popup-tags", &popup).set_raw_value(&tags.join(", "));
                        make_input("License", "popup-license", &popup).set_raw_value(&license);

                        let cover = document().get_element_by_id("cover").unwrap();
                        cover.parent_element().unwrap().remove_child(&cover).ok();
                        popup.append_child(&cover);

                        let button = document().create_element("button").unwrap();
                        button.append_child(&document().create_text_node("Publish"));
                        button.add_event_listener(mv!(widgets, old_ed => move |_: ClickEvent| {
                            console!(log, "wtf");
                            set_value("title", widgets.0.inner_text());
                            set_value("subtitle", widgets.1.inner_text());
                            console!(log, "là??");
                            set_value("editor-content", widgets.2.inner_text());
                            console!(log, "ici");
                            set_value("tags", get_elt_value("popup-tags"));
                            console!(log, "hein");
                            let cover = document().get_element_by_id("cover").unwrap();
                            cover.parent_element().unwrap().remove_child(&cover).ok();
                            old_ed.append_child(&cover);
                            console!(log, "d'eux");
                            set_value("license", get_elt_value("popup-license"));
                            console!(log, "ok ok");
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
