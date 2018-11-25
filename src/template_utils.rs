use plume_models::{Connection, posts::Post, users::User};
use rocket_i18n::Catalog;
use templates::Html;

pub enum Size {
    Small,
    Medium,
    Big,
}

impl Size {
    fn as_str(&self) -> &'static str {
        match self {
            Size::Small => "small",
            Size::Medium => "medium",
            Size::Big => "big",
        }
    }
}

pub fn avatar(conn: &Connection, user: &User, size: Size, pad: bool, catalog: &Catalog) -> Html<String> {
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
        title = i18n!(catalog, "{0}'s avatar"; user.name(conn)),
    ))
}

pub fn post_card(article: Post) -> Html<&'static str> {
    Html("todo")
}

/*{% macro post_card(article) %}
    <div class="card">
        {% if article.cover %}
            <div class="cover" style="background-image: url('{{ article.cover.url }}')"></div>
        {% endif %}
        <h3><a href="{{ article.url }}">{{ article.post.title }}</a></h3>
        <main>
            <p>
                {% if article.post.subtitle | length > 0 %}
                    {{ article.post.subtitle }}
                {% else %}
                    {{ article.post.content | safe | striptags | truncate(length=200) }}
                {% endif %}
            </p>
        </main>
        <p class="author">
        	{{ "By {{ link_1 }}{{ link_2 }}{{ link_3 }}{{ name | escape }}{{ link_4 }}" | _(
                link_1='<a href="/@/',
                link_2=article.author.fqn,
                link_3='/">',
                name=article.author.name,
                link_4="</a>")
            }}
            {% if article.post.published %}⋅ {{ article.date | date(format="%B %e") }}{% endif %}
            ⋅ <a href="/~/{{ article.blog.fqn }}/">{{ article.blog.title }}</a>
            {% if not article.post.published %}⋅ {{ "Draft" | _ }}{% endif %}
        </p>
    </div>
{% endmacro post_card %}*/

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
                error = if let Some(field) = $err.errors().get(stringify!($name)) {
                    if let ValidationErrorsKind::Field(errs) = field {
                        format!(r#"<p class="error">{}</p>"#, i18n!($catalog, &*errs[0].message.clone().unwrap_or(Cow::from("Unknown error"))))
                    } else {
                        String::new()
                    }
                } else {
                    String::new()
                },
                val = $form.$name,
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
    }
}
