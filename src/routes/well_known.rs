use rocket::http::ContentType;
use rocket::response::Content;

use activity_pub::webfinger::Webfinger;
use db_conn::DbConn;
use models::blogs::Blog;
use models::instance::Instance;
use models::users::User;

#[get("/.well-known/host-meta", format = "application/xml")]
fn host_meta(conn: DbConn) -> String {
    let domain = Instance::get_local(&*conn).unwrap().public_domain;
    format!(r#"
    <?xml version="1.0"?>
    <XRD xmlns="http://docs.oasis-open.org/ns/xri/xrd-1.0">
        <Link rel="lrdd" type="application/xrd+xml" template="https://{domain}/.well-known/webfinger?resource={{uri}}"/>
    </XRD>
    "#, domain = domain)
}

#[derive(FromForm)]
struct WebfingerQuery {
    resource: String
}

#[get("/.well-known/webfinger?<query>")]
fn webfinger(query: WebfingerQuery, conn: DbConn) -> Content<Result<String, &'static str>> {
    let mut parsed_query = query.resource.split(":");
    println!("{:?}", parsed_query.clone().collect::<Vec<&str>>());
    let res_type = parsed_query.next().unwrap();
    let res = parsed_query.next().unwrap();
    if res_type == "acct" {
        let mut parsed_res = res.split("@");
        let user = parsed_res.next().unwrap();
        let res_dom = parsed_res.next().unwrap();

        let domain = Instance::get_local(&*conn).unwrap().public_domain;

        if res_dom == domain {
            let res = match User::find_by_name(&*conn, String::from(user)) {
                Some(usr) => Ok(usr.webfinger(&*conn)),
                None => match Blog::find_by_actor_id(&*conn, String::from(user)) {
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
