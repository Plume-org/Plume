use models::instance::Instance;
use db_conn::DbConn;
use models::users::User;
use models::blogs::Blog;
use rocket_contrib::Json;
use activity_pub::webfinger::Webfinger;

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

#[get("/.well-known/webfinger?<query>", format = "application/jrd+json")]
fn webfinger_json(query: WebfingerQuery, conn: DbConn) -> Result<String, &'static str> {
    webfinger(query, conn, "json")
}

#[get("/.well-known/webfinger?<query>", format = "application/xrd+xml")]
fn webfinger_xml(query: WebfingerQuery, conn: DbConn) -> Result<String, &'static str> {
    webfinger(query, conn, "xml")
}

fn webfinger(query: WebfingerQuery, conn: DbConn, format: &'static str) -> Result<String, &'static str> {
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
            match User::find_by_name(&*conn, String::from(user)) {
                Some(usr) => Ok(usr.webfinger(format, &*conn)),
                None => match Blog::find_by_actor_id(&*conn, String::from(user)) {
                    Some(blog) => Ok(blog.webfinger(format, &*conn)),
                    None => Err("Requested actor not found")
                }
            }
        } else {
            Err("Invalid instance")
        }
    } else {
        Err("Invalid resource type. Only acct is supported")
    }
}
