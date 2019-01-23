#![recursion_limit="128"]
#[macro_use]
extern crate stdweb;

use stdweb::{unstable::{TryFrom, TryInto}, web::{*, event::*}};

fn main() {
    editor_loop();
    menu();
    search();
}

/// Auto expands the editor when adding text and count chars
fn editor_loop() {
    match document().query_selector("#plume-editor") {
        Ok(Some(x)) => HtmlElement::try_from(x).map(|article_content| {
            let offset = article_content.offset_height() - (article_content.get_bounding_client_rect().get_height() as i32);
            article_content.add_event_listener(move |_: KeyDownEvent| {
                let article_content = document().query_selector("#plume-editor").ok();
                js! {
                    @{&article_content}.style.height = "auto";
                    @{&article_content}.style.height = @{&article_content}.scrollHeight - @{offset} + "px";
                };
                window().set_timeout(|| {match document().query_selector("#post-form") {
                    Ok(Some(form)) => HtmlElement::try_from(form).map(|form| {
                        if let Some(len) = form.get_attribute("content-size").and_then(|s| s.parse::<i32>().ok()) {
                            let consumed: i32 = js!{
                                var len = - 1;
                                for(var i = 0; i < @{&form}.length; i++) {
                                    if(@{&form}[i].name != "") {
                                        len += @{&form}[i].name.length + encodeURIComponent(@{&form}[i].value)
                                            .replace(/%20/g, "+")
                                            .replace(/%0A/g, "%0D%0A")
                                            .replace(new RegExp("[!'*()]", "g"), "XXX") //replace exceptions of encodeURIComponent with placeholder
                                            .length + 2;
                                    }
                                }
                                return len;
                            }.try_into().unwrap_or_default();
                            match document().query_selector("#editor-left") {
                                Ok(Some(e)) => HtmlElement::try_from(e).map(|e| {
                                    js!{@{e}.innerText = (@{len-consumed})};
                                }).ok(),
                                _ => None,
                            };
                           }
                        }).ok(),
                    _ => None,
                };}, 0);
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
