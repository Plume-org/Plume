use plume_models::{Connection, users::User};
use rocket::response::Content;
use rocket_i18n::Catalog;
use templates::Html;

pub use askama_escape::escape;

pub type BaseContext<'a> = &'a(&'a Connection, &'a Catalog, Option<User>);

pub type Ructe = Content<Vec<u8>>;

#[macro_export]
macro_rules! render {
    ($group:tt :: $page:tt ( $( $param:expr ),* ) ) => {
        {
            use rocket::{http::ContentType, response::Content};
            use templates;

            let mut res = vec![];
            templates::$group::$page(
                &mut res,
                $(
                    $param
                ),*
            ).unwrap();
            Content(ContentType::HTML, res)
        }
    }
}

pub enum Size {
    Small,
    Medium,
}

impl Size {
    fn as_str(&self) -> &'static str {
        match self {
            Size::Small => "small",
            Size::Medium => "medium",
        }
    }
}

pub fn avatar(conn: &Connection, user: &User, size: Size, pad: bool, catalog: &Catalog) -> Html<String> {
    let name = escape(&user.name(conn)).to_string();
    Html(format!(
        r#"<div
        class="avatar {size} {padded}"
        style="background-image: url('{url}');"
        title="{title}"
        aria-label="{title}"
        ></div>"#,
        size = size.as_str(),
        padded = if pad { "padded" } else { "" },
        url = user.avatar_url(conn),
        title = i18n!(catalog, "{0}'s avatar"; name),
    ))
}

pub fn tabs(links: &[(&str, &str, bool)]) -> Html<String> {
    let mut res = String::from(r#"<div class="tabs">"#);
    for (url, title, selected) in links {
        res.push_str(r#"<a href=""#);
        res.push_str(url);
        if *selected {
            res.push_str(r#"" class="selected">"#);
        } else {
            res.push_str("\">");
        }
        res.push_str(title);
        res.push_str("</a>");
    }
    res.push_str("</div>");
    Html(res)
}

pub fn paginate(catalog: &Catalog, page: i32, total: i32) -> Html<String> {
    let mut res = String::new();
    res.push_str(r#"<div class="pagination">"#);
    if page != 1 {
        res.push_str(format!(r#"<a href="?page={}">{}</a>"#, page - 1, catalog.gettext("Previous page")).as_str());
    }
    if page < total {
        res.push_str(format!(r#"<a href="?page={}">{}</a>"#, page + 1, catalog.gettext("Next page")).as_str());
    }
    res.push_str("</div>");
    Html(res)
}

#[macro_export]
macro_rules! icon {
    ($name:expr) => {
        Html(concat!(r#"<svg class="feather"><use xlink:href="/static/images/feather-sprite.svg#"#, $name, "\"/></svg>"))
    }
}

macro_rules! input {
    ($catalog:expr, $name:tt ($kind:tt), $label:expr, $optional:expr, $details:expr, $form:expr, $err:expr, $props:expr) => {
        {
            use validator::ValidationErrorsKind;
            use std::borrow::Cow;

            Html(format!(r#"
                <label for="{name}">
                    {label}
                    {optional}
                    {details}
                </label>
                {error}
                <input type="{kind}" id="{name}" name="{name}" value="{val}" {props}/>
                "#,
                name = stringify!($name),
                label = i18n!($catalog, $label),
                kind = stringify!($kind),
                optional = if $optional { format!("<small>{}</small>", i18n!($catalog, "Optional")) } else { String::new() },
                details = if $details.len() > 0 {
                    format!("<small>{}</small>", i18n!($catalog, $details))
                } else {
                    String::new()
                },
                error = if let Some(ValidationErrorsKind::Field(errs)) = $err.errors().get(stringify!($name)) {
                    format!(r#"<p class="error">{}</p>"#, i18n!($catalog, &*errs[0].message.clone().unwrap_or(Cow::from("Unknown error"))))
                } else {
                    String::new()
                },
                val = escape(&$form.$name),
                props = $props
            ))
        }
    };
    ($catalog:expr, $name:tt (optional $kind:tt), $label:expr, $details:expr, $form:expr, $err:expr, $props:expr) => {
        input!($catalog, $name ($kind), $label, true, $details, $form, $err, $props)
    };
    ($catalog:expr, $name:tt (optional $kind:tt), $label:expr, $form:expr, $err:expr, $props:expr) => {
        input!($catalog, $name ($kind), $label, true, "", $form, $err, $props)
    };
    ($catalog:expr, $name:tt ($kind:tt), $label:expr, $details:expr, $form:expr, $err:expr, $props:expr) => {
        input!($catalog, $name ($kind), $label, false, $details, $form, $err, $props)
    };
    ($catalog:expr, $name:tt ($kind:tt), $label:expr, $form:expr, $err:expr, $props:expr) => {
        input!($catalog, $name ($kind), $label, false, "", $form, $err, $props)
    };
    ($catalog:expr, $name:tt ($kind:tt), $label:expr, $form:expr, $err:expr) => {
        input!($catalog, $name ($kind), $label, false, "", $form, $err, "")
    };
    ($catalog:expr, $name:tt ($kind:tt), $label:expr, $props:expr) => {
        {
            Html(format!(r#"
                <label for="{name}">{label}</label>
                <input type="{kind}" id="{name}" name="{name}" {props}/>
                "#,
                name = stringify!($name),
                label = i18n!($catalog, $label),
                kind = stringify!($kind),
                props = $props
            ))
        }
    };
}
