use plume_models::{notifications::*, users::User, Connection, PlumeRocket};

use rocket::http::hyper::header::{ETag, EntityTag};
use rocket::http::{Method, Status};
use rocket::request::Request;
use rocket::response::{self, content::Html as HtmlCt, Responder, Response};
use rocket_i18n::Catalog;
use std::collections::hash_map::DefaultHasher;
use std::hash::Hasher;
use templates::Html;

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
    fn respond_to(self, r: &Request) -> response::Result<'r> {
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
                .header(ETag(EntityTag::strong(etag)))
                .ok()
        } else {
            Response::build()
                .merge(HtmlCt(self.0).respond_to(r)?)
                .header(ETag(EntityTag::strong(etag)))
                .ok()
        }
    }
}

#[macro_export]
macro_rules! render {
    ($group:tt :: $page:tt ( $( $param:expr ),* ) ) => {
        {
            use templates;

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

pub fn translate_notification(ctx: BaseContext, notif: Notification) -> String {
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

pub fn tabs(links: &[(&str, String, bool)]) -> Html<String> {
    let mut res = String::from(r#"<div class="tabs">"#);
    for (url, title, selected) in links {
        res.push_str(r#"<a dir="auto" href=""#);
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
                catalog.gettext("Previous page")
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
                catalog.gettext("Next page")
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

macro_rules! input {
    ($catalog:expr, $name:tt ($kind:tt), $label:expr, $optional:expr, $details:expr, $form:expr, $err:expr, $props:expr) => {{
        use std::borrow::Cow;
        use validator::ValidationErrorsKind;
        let cat = $catalog;

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
            name = stringify!($name),
            label = i18n!(cat, $label),
            kind = stringify!($kind),
            optional = if $optional {
                format!("<small>{}</small>", i18n!(cat, "Optional"))
            } else {
                String::new()
            },
            details = if $details.len() > 0 {
                format!("<small>{}</small>", i18n!(cat, $details))
            } else {
                String::new()
            },
            error = if let Some(ValidationErrorsKind::Field(errs)) =
                $err.errors().get(stringify!($name))
            {
                format!(
                    r#"<p class="error" dir="auto">{}</p>"#,
                    errs[0]
                        .message
                        .clone()
                        .unwrap_or(Cow::from("Unknown error"))
                )
            } else {
                String::new()
            },
            val = escape(&$form.$name),
            props = $props
        ))
    }};
    ($catalog:expr, $name:tt (optional $kind:tt), $label:expr, $details:expr, $form:expr, $err:expr, $props:expr) => {
        input!(
            $catalog,
            $name($kind),
            $label,
            true,
            $details,
            $form,
            $err,
            $props
        )
    };
    ($catalog:expr, $name:tt (optional $kind:tt), $label:expr, $form:expr, $err:expr, $props:expr) => {
        input!(
            $catalog,
            $name($kind),
            $label,
            true,
            "",
            $form,
            $err,
            $props
        )
    };
    ($catalog:expr, $name:tt ($kind:tt), $label:expr, $details:expr, $form:expr, $err:expr, $props:expr) => {
        input!(
            $catalog,
            $name($kind),
            $label,
            false,
            $details,
            $form,
            $err,
            $props
        )
    };
    ($catalog:expr, $name:tt ($kind:tt), $label:expr, $form:expr, $err:expr, $props:expr) => {
        input!(
            $catalog,
            $name($kind),
            $label,
            false,
            "",
            $form,
            $err,
            $props
        )
    };
    ($catalog:expr, $name:tt ($kind:tt), $label:expr, $form:expr, $err:expr) => {
        input!($catalog, $name($kind), $label, false, "", $form, $err, "")
    };
    ($catalog:expr, $name:tt ($kind:tt), $label:expr, $props:expr) => {{
        let cat = $catalog;
        Html(format!(
            r#"
                <label for="{name}" dir="auto">{label}</label>
                <input type="{kind}" id="{name}" name="{name}" {props} dir="auto"/>
                "#,
            name = stringify!($name),
            label = i18n!(cat, $label),
            kind = stringify!($kind),
            props = $props
        ))
    }};
}

/// This macro imitate rocket's uri!, but with support for custom domains
///
/// It takes one more argument, domain, which must appear first, and must be an Option<&str>
/// sample call :
/// assuming both take the same parameters
/// url!(custom_domain=Some("something.tld"), posts::details: slug = "title", responding_to = _, blog = "blogname"));
///
/// assuming posts::details take one more parameter than posts::custom::details
/// url!(custom_domain=Some("something.tld"), posts::details:
///          common=[slug = "title", responding_to = _],
///          normal=[blog = "blogname"]));
///
/// you can also provide custom=[] for custom-domain specific arguments
/// custom_domain can be changed to anything, indicating custom domain varname in the custom-domain
/// function (most likely custom_domain or _custom_domain)
macro_rules! url {
    ($custom_domain:ident=$domain:expr, $module:ident::$route:ident:
        common=[$($common_args:ident = $common_val:expr),*],
        normal=[$($normal_args:ident = $normal_val:expr),*],
        custom=[$($custom_args:ident = $custom_val:expr),*]) => {{
        let domain: Option<&str> = $domain; //for type inference with None
        if let Some(domain) = domain {
            let origin = uri!(crate::routes::$module::custom::$route:
                              $custom_domain=&domain,
                              $($common_args = $common_val,)*
                              $($custom_args = $custom_val,)*
                              );
            let path = origin.segments().skip(1).map(|seg| format!("/{}", seg)).collect::<String>(); //first segment is domain, drop it
            let query = origin.query().map(|q| format!("?{}", q)).unwrap_or_default();
            format!("https://{}{}{}", &domain, path, query)
        } else {
            url!($module::$route:
                 $($common_args = $common_val,)*
                 $($normal_args = $normal_val,)*)
                .to_string()
        }
    }};
    ($cd:ident=$d:expr, $m:ident::$r:ident:
        common=[$($tt:tt)*]) => {
        url!($cd=$d, $m::$r: common=[$($tt)*], normal=[], custom=[])
    };
    ($cd:ident=$d:expr, $m:ident::$r:ident:
        normal=[$($tt:tt)*]) => {
        url!($cd=$d, $m::$r: common=[], normal=[$($tt)*], custom=[])
    };
    ($cd:ident=$d:expr, $m:ident::$r:ident:
        custom=[$($tt:tt)*]) => {
        url!($cd=$d, $m::$r: common=[], normal=[], custom=[$($tt)*])
    };
    ($cd:ident=$d:expr, $m:ident::$r:ident:
        common=[$($co:tt)*],
        normal=[$($no:tt)*]) => {
        url!($cd=$d, $m::$r: common=[$($co)*], normal=[$($no)*], custom=[])
    };
    ($cd:ident=$d:expr, $m:ident::$r:ident:
        common=[$($co:tt)*],
        custom=[$($cu:tt)*]) => {
        url!($cd=$d, $m::$r: common=[$($co)*], normal=[], custom=[$($cu)*])
    };
    ($cd:ident=$d:expr, $m:ident::$r:ident:
        normal=[$($no:tt)*],
        custom=[$($cu:tt)*]) => {
        url!($cd=$d, $m::$r: common=[], normal=[$($no)*], custom=[$($cu)*])
    };
    ($custom_domain:ident=$domain:expr, $module:ident::$route:ident: $($common_args:tt)*) => {
        url!($custom_domain=$domain, $module::$route: common=[$($common_args)*])
    };
    ($module:ident::$route:ident: $($tt:tt)*) => {
            uri!(crate::routes::$module::$route: $($tt)*)
    };
}
