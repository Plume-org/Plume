static ref CATALOG: Arc<RefCell<Option<Catalog>>> = Arc::new(RefCell::New(None));

fn load_mo() {
    if window().session_storage().contains_key("plume-mo") {
        *CATALOG.borrow_mut() = Some(Catalog::parse(window().session_storage().get("plume-mo").unwrap()))
    } else {
        let xhr = XmlHttpRequest::new();
        xhr.open("/static/plume-front.mo");
        xhr.add_event_listener(move |e: ReadyStateChangeEvent| {
            match xhr.ready_state() {
                Done => {
                    let res = xhr.response_text().unwrap().unwrap();
                    *CATALOG.borrow_mut() = Some(Catalog::parse(res));
                    window().session_storage().insert("plume-mo", res);
                },
                _ => {},
            }
        });
        xhr.send();
    }
}

#[macro_export]
macro_rules! i18n {
    ( $( $arg:tt )+ ) => {
        i18n!($crate::i18n::CATALOG.borrow().unwrap(), $( $arg )+ )
    }
}
