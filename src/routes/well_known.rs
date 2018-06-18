use rocket::http::ContentType;
use rocket::response::Content;
use serde_json;
use webfinger::*;

use BASE_URL;
use activity_pub::ap_url;
use db_conn::DbConn;
use models::{blogs::Blog, users::User};

#[get("/.well-known/nodeinfo")]
fn nodeinfo() -> Content<String> {
    Content(ContentType::new("application", "jrd+json"), json!({
        "links": [
            {
                "rel": "http://nodeinfo.diaspora.software/ns/schema/2.0",
                "href": ap_url(format!("{domain}/nodeinfo", domain = BASE_URL.as_str()))
            }
        ]
    }).to_string())
}

#[get("/.well-known/host-meta", format = "application/xml")]
fn host_meta() -> String {
    format!(r#"
    <?xml version="1.0"?>
    <XRD xmlns="http://docs.oasis-open.org/ns/xri/xrd-1.0">
        <Link rel="lrdd" type="application/xrd+xml" template="{url}"/>
    </XRD>
    "#, url = ap_url(format!("{domain}/.well-known/webfinger?resource={{uri}}", domain = BASE_URL.as_str())))
}

#[derive(FromForm)]
struct WebfingerQuery {
    resource: String
}

struct WebfingerResolver;

impl Resolver<DbConn> for WebfingerResolver {
    fn instance_domain<'a>() -> &'a str {
        BASE_URL.as_str()
    }

    fn find(acct: String, conn: DbConn) -> Result<Webfinger, ResolverError> {
        match User::find_local(&*conn, acct.clone()) {
            Some(usr) => Ok(usr.webfinger(&*conn)),
            None => match Blog::find_local(&*conn, acct) {
                Some(blog) => Ok(blog.webfinger(&*conn)),
                None => Err(ResolverError::NotFound)
            }
        }
    }
}

#[get("/.well-known/webfinger?<query>")]
fn webfinger(query: WebfingerQuery, conn: DbConn) -> Content<String> {
    match WebfingerResolver::endpoint(query.resource, conn).and_then(|wf| serde_json::to_string(&wf).map_err(|_| ResolverError::NotFound)) {
        Ok(wf) => Content(ContentType::new("application", "jrd+json"), wf),
        Err(err) => Content(ContentType::new("text", "plain"), String::from(match err {
            ResolverError::InvalidResource => "Invalid resource. Make sure to request an acct: URI",
            ResolverError::NotFound => "Requested resource was not found",
            ResolverError::WrongInstance => "This is not the instance of the requested resource"
        }))
    }
}
