#![recursion_limit = "128"]
#![feature(decl_macro, proc_macro_hygiene)]

#[macro_use]
extern crate gettext_macros;
#[macro_use]
extern crate lazy_static;

use wasm_bindgen::{prelude::*, JsCast};
use web_sys::{console, window, Document, Element, Event, HtmlInputElement, TouchEvent};

init_i18n!(
    "plume-front",
    af,
    ar,
    bg,
    ca,
    cs,
    cy,
    da,
    de,
    el,
    en,
    eo,
    es,
    fa,
    fi,
    fr,
    gl,
    he,
    hi,
    hr,
    hu,
    it,
    ja,
    ko,
    nb,
    nl,
    no,
    pl,
    pt,
    ro,
    ru,
    sat,
    si,
    sk,
    sl,
    sr,
    sv,
    tr,
    uk,
    vi,
    zh
);

mod editor;

compile_i18n!();

lazy_static! {
    static ref CATALOG: gettext::Catalog = {
        let catalogs = include_i18n!();
        let lang = window().unwrap().navigator().language().unwrap();
        let lang = lang.split_once('-').map_or("en", |x| x.0);

        let english_position = catalogs
            .iter()
            .position(|(language_code, _)| *language_code == "en")
            .unwrap();
        catalogs
            .iter()
            .find(|(l, _)| l == &lang)
            .unwrap_or(&catalogs[english_position])
            .clone()
            .1
    };
}

#[wasm_bindgen(start)]
pub fn main() -> Result<(), JsValue> {
    extern crate console_error_panic_hook;
    use std::panic;
    panic::set_hook(Box::new(console_error_panic_hook::hook));

    menu();
    search();
    editor::init()
        .map_err(|e| console::error_1(&format!("Editor error: {:?}", e).into()))
        .ok();
    Ok(())
}

/// Toggle menu on mobile devices
///
/// It should normally be working fine even without this code
/// But :focus-within is not yet supported by Webkit/Blink
fn menu() {
    let document = document();
    if let Ok(Some(button)) = document.query_selector("#menu a") {
        if let Some(menu) = document.get_element_by_id("content") {
            let show_menu = Closure::wrap(Box::new(|_: TouchEvent| {
                self::document()
                    .get_element_by_id("menu")
                    .map(|menu| {
                        menu.set_attribute("aria-expanded", "true")
                            .map(|_| menu.class_list().add_1("show"))
                    })
                    .unwrap()
                    .unwrap()
                    .unwrap();
            }) as Box<dyn FnMut(TouchEvent)>);
            button
                .add_event_listener_with_callback("touchend", show_menu.as_ref().unchecked_ref())
                .unwrap();
            show_menu.forget();

            let close_menu = Closure::wrap(Box::new(|evt: TouchEvent| {
                if evt
                    .target()
                    .unwrap()
                    .dyn_ref::<Element>()
                    .unwrap()
                    .closest("a")
                    .unwrap()
                    .is_some()
                {
                    return;
                }
                self::document()
                    .get_element_by_id("menu")
                    .map(|menu| {
                        menu.set_attribute("aria-expanded", "false")
                            .map(|_| menu.class_list().remove_1("show"))
                    })
                    .unwrap()
                    .unwrap()
                    .unwrap()
            }) as Box<dyn FnMut(TouchEvent)>);
            menu.add_event_listener_with_callback("touchend", close_menu.as_ref().unchecked_ref())
                .unwrap();
            close_menu.forget();
        }
    }
}

/// Clear the URL of the search page before submitting request
fn search() {
    if let Some(form) = document().get_element_by_id("form") {
        let normalize_query = Closure::wrap(Box::new(|_: Event| {
            document()
                .query_selector_all("#form input")
                .map(|inputs| {
                    for i in 0..inputs.length() {
                        let input = inputs.get(i).unwrap();
                        let input = input.dyn_ref::<HtmlInputElement>().unwrap();
                        if input.name().is_empty() {
                            input.set_name(&input.dyn_ref::<Element>().unwrap().id());
                        }
                        if !input.name().is_empty() && input.value().is_empty() {
                            input.set_name("");
                        }
                    }
                })
                .unwrap();
        }) as Box<dyn FnMut(Event)>);
        form.add_event_listener_with_callback("submit", normalize_query.as_ref().unchecked_ref())
            .unwrap();
        normalize_query.forget();
    }
}

fn document() -> Document {
    window().unwrap().document().unwrap()
}
