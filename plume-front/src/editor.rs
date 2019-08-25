use pulldown_cmark::{Event, Options, Parser, Tag};
use stdweb::{
    unstable::{TryFrom, TryInto},
    web::{event::*, html_element::*, *},
};
use CATALOG;

fn from_md(md: &str) {
    let md_parser = Parser::new_ext(md, Options::all());
    md_parser.fold(
        document().get_element_by_id("editor-main").unwrap(),
        |last_elt, event| {
            match event {
                Event::Start(tag) => {
                    let new = match tag {
                        Tag::Paragraph => document().create_element("p").unwrap(),
                        Tag::Rule => document().create_element("hr").unwrap(),
                        Tag::Header(level) => {
                            document().create_element(&format!("h{}", level)).unwrap()
                        }
                        Tag::BlockQuote => document().create_element("blockquote").unwrap(),
                        Tag::CodeBlock(code) => {
                            let pre = document().create_element("pre").unwrap();
                            let code_elt = document().create_element("code").unwrap();
                            code_elt.append_child(&document().create_text_node(&code));
                            pre.append_child(&code_elt);
                            pre
                        }
                        Tag::List(None) => document().create_element("ul").unwrap(),
                        Tag::List(Some(_start_index)) => document().create_element("ol").unwrap(), // TODO: handle start_index
                        Tag::Item => document().create_element("li").unwrap(),
                        Tag::FootnoteDefinition(def) => {
                            let note = document().create_element("div").unwrap();
                            note.class_list().add("footnote");
                            note.append_child(&document().create_text_node(&def));
                            note
                        }
                        Tag::HtmlBlock => document().create_element("div").unwrap(),
                        Tag::Table(_alignements) => document().create_element("table").unwrap(), // TODO: handle alignements
                        Tag::TableHead => document().create_element("th").unwrap(),
                        Tag::TableRow => document().create_element("tr").unwrap(),
                        Tag::TableCell => document().create_element("td").unwrap(),
                        Tag::Emphasis => document().create_element("em").unwrap(),
                        Tag::Strong => document().create_element("strong").unwrap(),
                        Tag::Strikethrough => document().create_element("s").unwrap(),
                        Tag::Link(_link_type, url, text) => {
                            let url: &str = &url;
                            let text: &str = &text;
                            let link = document().create_element("a").unwrap();
                            js! {
                                @{&link}.href = @{url};
                                @{&link}.title = @{text};
                            };
                            link
                        }
                        Tag::Image(_link_type, url, text) => {
                            let url: &str = &url;
                            let text: &str = &text;
                            let img = document().create_element("img").unwrap();
                            js! {
                                @{&img}.src = @{url};
                                @{&img}.title = @{text};
                                @{&img}.alt = @{text};
                            };
                            img
                        }
                    };
                    last_elt.append_child(&new);
                    new
                }
                Event::End(_) => last_elt.parent_element().unwrap(),
                Event::Text(text) => {
                    let node = document().create_text_node(&text);
                    last_elt.append_child(&node);
                    last_elt
                }
                Event::Code(code) => {
                    let elt = document().create_element("code").unwrap();
                    let content = document().create_text_node(&code);
                    elt.append_child(&content);
                    last_elt.append_child(&elt);
                    last_elt
                }
                Event::Html(html) => {
                    // TODO: sanitize it?
                    last_elt.set_attribute("innerHtml", &html);
                    last_elt
                }
                Event::InlineHtml(html) => {
                    let elt = document().create_element("span").unwrap();
                    elt.set_attribute("innerHtml", &html);
                    last_elt.append_child(&elt);
                    last_elt
                }
                Event::FootnoteReference(reference) => {
                    last_elt // TODO
                }
                Event::SoftBreak => {
                    last_elt.append_child(&document().create_element("br").unwrap());
                    last_elt
                }
                Event::HardBreak => {
                    last_elt // TODO
                }
                Event::TaskListMarker(done) => {
                    last_elt // TODO
                }
            }
        },
    );

    MutationObserver::new(|muts, _obs| {
        for m in muts {
            console!(log, "mut!!");
        }
    })
    .observe(
        &document().get_element_by_id("editor-main").unwrap(),
        MutationObserverInit {
            child_list: true,
            attributes: true,
            character_data: false,
            subtree: true,
            attribute_old_value: true,
            character_data_old_value: false,
            attribute_filter: None,
        },
    );
}

fn to_md() -> String {
    let root = document().get_element_by_id("editor-main").unwrap();
    fold_children(&root).join("")
}

fn fold_children(elt: &Element) -> Vec<String> {
    elt.child_nodes().iter().fold(vec![], |mut blocks, node| {
        blocks.push(html_to_md(&node));
        blocks
    })
}

fn html_to_md(node: &Node) -> String {
    console!(log, node);
    if let Ok(elt) = Element::try_from(node.clone()) {
        console!(log, elt.node_name().to_lowercase());
        match elt.node_name().to_lowercase().as_ref() {
            "hr" => "---".into(),
            "h1" => format!("# {}\n\n", fold_children(&elt).join("")),
            "h2" => format!("## {}\n\n", fold_children(&elt).join("")),
            "h3" => format!("### {}\n\n", fold_children(&elt).join("")),
            "h4" => format!("#### {}\n\n", fold_children(&elt).join("")),
            "h5" => format!("##### {}\n\n", fold_children(&elt).join("")),
            "h6" => format!("###### {}\n\n", fold_children(&elt).join("")),
            "blockquote" => format!("> {}\n\n", fold_children(&elt).join("> ")),
            "pre" => format!("```\n{}\n```\n\n", node.text_content().unwrap_or_default()),
            "li" => match elt
                .parent_element()
                .unwrap()
                .node_name()
                .to_lowercase()
                .as_ref()
            {
                "ol" => format!(
                    "{}. {}\n",
                    elt.parent_element()
                        .unwrap()
                        .child_nodes()
                        .iter()
                        .position(|n| Element::try_from(n).unwrap() == elt)
                        .unwrap_or_default(),
                    fold_children(&elt).join(""),
                ),
                _ => format!("- {}\n", fold_children(&elt).join("")),
            },
            "em" => format!("_{}_", fold_children(&elt).join("")),
            "strong" => format!("**{}**", fold_children(&elt).join("")),
            "s" => format!("~~{}~~", fold_children(&elt).join("")),
            "a" => format!(
                "[{}]({})",
                fold_children(&elt).join(""),
                String::try_from(js! { return @{&elt}.href }).unwrap()
            ),
            "img" => format!(
                "![{}]({})",
                String::try_from(js! { return @{&elt}.alt }).unwrap(),
                String::try_from(js! { return @{&elt}.src }).unwrap()
            ),
            other => {
                console!(log, "Warning: unhandled element:", other);
                String::new()
            } // TODO: refs, tables, raw html
        }
    } else {
        node.text_content().unwrap_or_default()
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
        let source = get_elt_value("editor-content");

        setup_toolbar();
        from_md(&source);

        title.add_event_listener(no_return);
        subtitle.add_event_listener(no_return);

        filter_paste(&title);
        filter_paste(&subtitle);
        // TODO: filter_paste(&content);

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

fn select_style(style: &str) {
    if let Some(select) = document()
        .get_element_by_id("toolbar-style")
        .and_then(|e| SelectElement::try_from(e).ok())
    {
        select.set_value(Some(style));
    }
}

fn setup_toolbar() {
    let toolbar = document().get_element_by_id("editor-toolbar").unwrap();

    // List of styles (headings, quote, code, etc)
    let style_select =
        SelectElement::try_from(document().create_element("select").unwrap()).unwrap();
    let options = vec![
        ("p", i18n!(CATALOG, "Paragraph")),
        ("ul", i18n!(CATALOG, "List")),
        ("ol", i18n!(CATALOG, "Ordered list")),
        ("h1", i18n!(CATALOG, "Heading 1")),
        ("h2", i18n!(CATALOG, "Heading 2")),
        ("h3", i18n!(CATALOG, "Heading 3")),
        ("h4", i18n!(CATALOG, "Heading 4")),
        ("h5", i18n!(CATALOG, "Heading 5")),
        ("h6", i18n!(CATALOG, "Heading 6")),
        ("blockquote", i18n!(CATALOG, "Quote")),
        ("pre", i18n!(CATALOG, "Code")),
    ];
    for (tag, name) in options.clone() {
        let opt = document().create_element("option").unwrap();
        opt.set_attribute("value", tag);
        opt.append_child(&document().create_text_node(&name));
        style_select.append_child(&opt)
    }
    style_select.set_attribute("id", "toolbar-style");

    let options_clone = options.clone();
    document().add_event_listener(move |_: SelectionChangeEvent| {
        let block = std::iter::successors(
            window().get_selection().and_then(|s| s.anchor_node()),
            |node| {
                let t = node.node_name().to_lowercase();
                if options_clone.iter().any(|(tag, _)| *tag == &t) {
                    None
                } else {
                    node.parent_node()
                }
            },
        )
        .last();

        if let Some(b) = block {
            select_style(&b.node_name().to_lowercase());
        }
    });

    style_select.add_event_listener(move |_: ChangeEvent| {
        let block = std::iter::successors(
            window().get_selection().and_then(|s| s.anchor_node()),
            |node| {
                let t = node.node_name().to_lowercase();
                if options.iter().any(|(tag, _)| *tag == &t) {
                    None
                } else {
                    node.parent_node()
                }
            },
        )
        .last();

        if let Some(block) = block {
            if let Some(select) = document()
                .get_element_by_id("toolbar-style")
                .and_then(|e| SelectElement::try_from(e).ok())
            {
                let tag = select.value();

                let new = document().create_element(&tag.unwrap_or_default()).unwrap();
                for ch in block.child_nodes() {
                    block.remove_child(&ch);
                    new.append_child(&ch);
                }

                block.parent_node().unwrap().replace_child(&new, &block);
            }
        }
    });

    // Bold

    // Italics

    // Insert an image

    toolbar.append_child(&style_select);
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
    console!(log, to_md());
    let data = plume_api::posts::NewPostData {
        title: HtmlElement::try_from(document().get_element_by_id("editor-title").unwrap())
            .unwrap()
            .inner_text(),
        subtitle: document()
            .get_element_by_id("editor-subtitle")
            .map(|s| HtmlElement::try_from(s).unwrap().inner_text()),
        source: to_md(),
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
