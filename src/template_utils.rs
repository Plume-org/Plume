use plume_models::{notifications::*, users::User, Connection, PlumeRocket};

use crate::templates::Html;
use rocket::http::{Method, Status};
use rocket::request::Request;
use rocket::response::{self, content::Html as HtmlCt, Responder, Response};
use rocket_i18n::Catalog;
use std::collections::{btree_map::BTreeMap, hash_map::DefaultHasher};
use std::hash::Hasher;

pub use askama_escape::escape;

pub static CACHE_NAME: &str = env!("CACHE_ID");

pub type BaseContext<'a> = &'a (
    &'a Connection,
    &'a Catalog,
    Option<User>,
    Option<(String, String)>,
);

pub trait IntoContext {
    fn to_context(
        &self,
    ) -> (
        &Connection,
        &Catalog,
        Option<User>,
        Option<(String, String)>,
    );
}

impl IntoContext for PlumeRocket {
    fn to_context(
        &self,
    ) -> (
        &Connection,
        &Catalog,
        Option<User>,
        Option<(String, String)>,
    ) {
        (
            &*self.conn,
            &self.intl.catalog,
            self.user.clone(),
            self.flash_msg.clone(),
        )
    }
}

#[derive(Debug)]
pub struct Ructe(pub Vec<u8>);

impl<'r> Responder<'r> for Ructe {
    fn respond_to(self, r: &Request<'_>) -> response::Result<'r> {
        //if method is not Get or page contain a form, no caching
        if r.method() != Method::Get || self.0.windows(6).any(|w| w == b"<form ") {
            return HtmlCt(self.0).respond_to(r);
        }
        let mut hasher = DefaultHasher::new();
        hasher.write(&self.0);
        let etag = format!("{:x}", hasher.finish());
        if r.headers()
            .get("If-None-Match")
            .any(|s| s[1..s.len() - 1] == etag)
        {
            Response::build()
                .status(Status::NotModified)
                .header("ETag", etag)
                .ok()
        } else {
            Response::build()
                .merge(HtmlCt(self.0).respond_to(r)?)
                .header("ETag", etag)
                .ok()
        }
    }
}

#[macro_export]
macro_rules! render {
    ($group:tt :: $page:tt ( $( $param:expr ),* ) ) => {
        {
            use crate::templates;

            let mut res = vec![];
            templates::$group::$page(
                &mut res,
                $(
                    $param
                ),*
            ).unwrap();
            Ructe(res)
        }
    }
}

pub fn translate_notification(ctx: BaseContext<'_>, notif: Notification) -> String {
    let name = notif.get_actor(ctx.0).unwrap().name();
    match notif.kind.as_ref() {
        notification_kind::COMMENT => i18n!(ctx.1, "{0} commented on your article."; &name),
        notification_kind::FOLLOW => i18n!(ctx.1, "{0} is subscribed to you."; &name),
        notification_kind::LIKE => i18n!(ctx.1, "{0} liked your article."; &name),
        notification_kind::MENTION => i18n!(ctx.1, "{0} mentioned you."; &name),
        notification_kind::RESHARE => i18n!(ctx.1, "{0} boosted your article."; &name),
        _ => unreachable!("translate_notification: Unknow type"),
    }
}

pub fn i18n_timeline_name(cat: &Catalog, tl: &str) -> String {
    match tl {
        "Your feed" => i18n!(cat, "Your feed"),
        "Local feed" => i18n!(cat, "Local feed"),
        "Federated feed" => i18n!(cat, "Federated feed"),
        n => n.to_string(),
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

pub fn avatar(
    conn: &Connection,
    user: &User,
    size: Size,
    pad: bool,
    catalog: &Catalog,
) -> Html<String> {
    let name = escape(&user.name()).to_string();
    Html(format!(
        r#"<div class="avatar {size} {padded}"
        style="background-image: url('{url}');"
        title="{title}"
        aria-label="{title}"></div>
        <img class="hidden u-photo" src="{url}"/>"#,
        size = size.as_str(),
        padded = if pad { "padded" } else { "" },
        url = user.avatar_url(conn),
        title = i18n!(catalog, "{0}'s avatar"; name),
    ))
}

pub fn tabs(links: &[(impl AsRef<str>, String, bool)]) -> Html<String> {
    let mut res = String::from(r#"<div class="tabs">"#);
    for (url, title, selected) in links {
        res.push_str(r#"<a dir="auto" href=""#);
        res.push_str(url.as_ref());
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
    paginate_param(catalog, page, total, None)
}
pub fn paginate_param(
    catalog: &Catalog,
    page: i32,
    total: i32,
    param: Option<String>,
) -> Html<String> {
    let mut res = String::new();
    let param = param
        .map(|mut p| {
            p.push('&');
            p
        })
        .unwrap_or_default();
    res.push_str(r#"<div class="pagination" dir="auto">"#);
    if page != 1 {
        res.push_str(
            format!(
                r#"<a href="?{}page={}">{}</a>"#,
                param,
                page - 1,
                i18n!(catalog, "Previous page")
            )
            .as_str(),
        );
    }
    if page < total {
        res.push_str(
            format!(
                r#"<a href="?{}page={}">{}</a>"#,
                param,
                page + 1,
                i18n!(catalog, "Next page")
            )
            .as_str(),
        );
    }
    res.push_str("</div>");
    Html(res)
}

pub fn encode_query_param(param: &str) -> String {
    param
        .chars()
        .map(|c| match c {
            '+' => Ok("%2B"),
            ' ' => Err('+'),
            c => Err(c),
        })
        .fold(String::new(), |mut s, r| {
            match r {
                Ok(r) => s.push_str(r),
                Err(r) => s.push(r),
            };
            s
        })
}

#[macro_export]
macro_rules! icon {
    ($name:expr) => {
        Html(concat!(
            r#"<svg class="feather"><use xlink:href="/static/images/feather-sprite.svg#"#,
            $name,
            "\"/></svg>"
        ))
    };
}

/// A builder type to generate `<input>` tags in a type-safe way.
///
/// # Example
///
/// This example uses all options, but you don't have to specify everything.
///
/// ```rust
/// # let current_email = "foo@bar.baz";
/// # let catalog = gettext::Catalog::parse("").unwrap();
/// Input::new("mail", "Your email address")
///     .input_type("email")
///     .default(current_email)
///     .optional()
///     .details("We won't use it for advertising.")
///     .set_prop("class", "email-input")
///     .to_html(catalog);
/// ```
pub struct Input {
    /// The name of the input (`name` and `id` in HTML).
    name: String,
    /// The description of this field.
    label: String,
    /// The `type` of the input (`text`, `email`, `password`, etc).
    input_type: String,
    /// The default value for this input field.
    default: Option<String>,
    /// `true` if this field is not required (will add a little badge next to the label).
    optional: bool,
    /// A small message to display next to the label.
    details: Option<String>,
    /// Additional HTML properties.
    props: BTreeMap<String, String>,
    /// The error message to show next to this field.
    error: Option<String>,
}

impl Input {
    /// Creates a new input with a given name.
    pub fn new(name: impl ToString, label: impl ToString) -> Input {
        Input {
            name: name.to_string(),
            label: label.to_string(),
            input_type: "text".into(),
            default: None,
            optional: false,
            details: None,
            props: BTreeMap::new(),
            error: None,
        }
    }

    /// Set the `type` of this input.
    pub fn input_type(mut self, t: impl ToString) -> Input {
        self.input_type = t.to_string();
        self
    }

    /// Marks this field as optional.
    pub fn optional(mut self) -> Input {
        self.optional = true;
        self
    }

    /// Fills the input with a default value (useful for edition form, to show the current values).
    pub fn default(mut self, val: impl ToString) -> Input {
        self.default = Some(val.to_string());
        self
    }

    /// Adds additional information next to the label.
    pub fn details(mut self, text: impl ToString) -> Input {
        self.details = Some(text.to_string());
        self
    }

    /// Defines an additional HTML property.
    ///
    /// This method can be called multiple times for the same input.
    pub fn set_prop(mut self, key: impl ToString, val: impl ToString) -> Input {
        self.props.insert(key.to_string(), val.to_string());
        self
    }

    /// Shows an error message
    pub fn error(mut self, errs: &validator::ValidationErrors) -> Input {
        if let Some(field_errs) = errs.clone().field_errors().get(self.name.as_str()) {
            self.error = Some(
                field_errs[0]
                    .message
                    .clone()
                    .unwrap_or_default()
                    .to_string(),
            );
        }
        self
    }

    /// Returns the HTML markup for this field.
    pub fn html(mut self, cat: &Catalog) -> Html<String> {
        if !self.optional {
            self = self.set_prop("required", true);
        }

        Html(format!(
            r#"
                <label for="{name}" dir="auto">
                    {label}
                    {optional}
                    {details}
                </label>
                {error}
                <input type="{kind}" id="{name}" name="{name}" value="{val}" {props} dir="auto"/>
                "#,
            name = self.name,
            label = self.label,
            kind = self.input_type,
            optional = if self.optional {
                format!("<small>{}</small>", i18n!(cat, "Optional"))
            } else {
                String::new()
            },
            details = self
                .details
                .map(|d| format!("<small>{}</small>", d))
                .unwrap_or_default(),
            error = self
                .error
                .map(|e| format!(r#"<p class="error" dir="auto">{}</p>"#, e))
                .unwrap_or_default(),
            val = escape(&self.default.unwrap_or_default()),
            props = self
                .props
                .into_iter()
                .fold(String::new(), |mut res, (key, val)| {
                    res.push_str(&format!("{}=\"{}\" ", key, val));
                    res
                })
        ))
    }
}
