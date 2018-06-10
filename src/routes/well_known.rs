use rocket::http::ContentType;
use rocket::response::Content;

use BASE_URL;
use activity_pub::{ap_url, webfinger::Webfinger};
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

#[get("/.well-known/webfinger?<query>")]
fn webfinger(query: WebfingerQuery, conn: DbConn) -> Content<Result<String, &'static str>> {
    let mut parsed_query = query.resource.splitn(2, ":");
    let res_type = parsed_query.next().unwrap();
    let res = parsed_query.next().unwrap();
    if res_type == "acct" {
        let mut parsed_res = res.split("@");
        let user = parsed_res.next().unwrap();
        let res_dom = parsed_res.next().unwrap();

        if res_dom == BASE_URL.as_str() {
            let res = match User::find_local(&*conn, String::from(user)) {
                Some(usr) => Ok(usr.webfinger(&*conn)),
                None => match Blog::find_local(&*conn, String::from(user)) {
                    Some(blog) => Ok(blog.webfinger(&*conn)),
                    None => Err("Requested actor not found")
                }
            };
            Content(ContentType::new("application", "jrd+json"), res)            
        } else {
            Content(ContentType::new("text", "plain"), Err("Invalid instance"))
        }
    } else {
        Content(ContentType::new("text", "plain"), Err("Invalid resource type. Only acct is supported"))
    }
}
