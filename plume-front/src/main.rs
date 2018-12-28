#![recursion_limit="128"]

#[macro_use]
extern crate stdweb;

use stdweb::{unstable::TryFrom, web::{*, event::*}};

mod editor;

fn main() {
    auto_expand();
    menu();
    search();
    editor::init();
}

/// Auto expands the editor when adding text
fn auto_expand() {
    match document().query_selector("#editor-content") {
        Ok(Some(x)) => HtmlElement::try_from(x).map(|article_content| {
            let offset = article_content.offset_height() - (article_content.get_bounding_client_rect().get_height() as i32);
            article_content.add_event_listener(move |_: KeyDownEvent| {
                let article_content = document().query_selector("#editor-content").ok();
                js! {
                    @{&article_content}.style.height = "auto";
                    @{&article_content}.style.height = @{&article_content}.scrollHeight - @{offset} + "px";
                }
            });
        }).ok(),
        _ => None
    };
}

/// Toggle menu on mobile device
///
/// It should normally be working fine even without this code
/// But :focus-within is not yet supported by Webkit/Blink
fn menu() {
    document().get_element_by_id("menu")
        .map(|button| {
            document().get_element_by_id("content")
                .map(|menu| {
                    button.add_event_listener(|_: ClickEvent| {
                        document().get_element_by_id("menu").map(|menu| menu.class_list().add("show"));
                    });
                    menu.add_event_listener(|_: ClickEvent| {
                        document().get_element_by_id("menu").map(|menu| menu.class_list().remove("show"));
                    });
                })
        });
}

/// Clear the URL of the search page before submitting request
fn search() {
    document().get_element_by_id("form")
        .map(|form| {
            form.add_event_listener(|_: SubmitEvent| {
                document().query_selector_all("#form input").map(|inputs| {
                    for input in inputs {
                        js! {
                            if (@{&input}.name === "") {
                                @{&input}.name = @{&input}.id
                            }
                            if (@{&input}.name && !@{&input}.value) {
                                @{&input}.name = "";
                            }
                        }
                    }
                }).ok();
            });
        });
}
