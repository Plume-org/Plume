use diesel::PgConnection;
use reqwest::Client;
use reqwest::header::{Accept, qitem};
use reqwest::mime::Mime;
use serde_json;

pub trait Webfinger {
    fn webfinger_subject(&self, conn: &PgConnection) -> String;
    fn webfinger_aliases(&self, conn: &PgConnection) -> Vec<String>;
    fn webfinger_links(&self, conn: &PgConnection) -> Vec<Vec<(String, String)>>;

    fn webfinger(&self, conn: &PgConnection) -> String {
        json!({
            "subject": self.webfinger_subject(conn),
            "aliases": self.webfinger_aliases(conn),
            "links": self.webfinger_links(conn).into_iter().map(|link| {
                let mut link_obj = serde_json::Map::new();
                for (k, v) in link {
                    link_obj.insert(k, serde_json::Value::String(v));
                }
                serde_json::Value::Object(link_obj)
            }).collect::<Vec<serde_json::Value>>()
        }).to_string()
    }
}

pub fn resolve(acct: String) -> Result<String, String> {
    let instance = acct.split("@").last().unwrap();
    let url = format!("https://{}/.well-known/webfinger?resource=acct:{}", instance, acct);
    Client::new()
        .get(&url[..])
        .header(Accept(vec![qitem("application/jrd+json".parse::<Mime>().unwrap())]))
        .send()
        .map(|mut r| {
            let res = r.text().unwrap();
            let json: serde_json::Value = serde_json::from_str(&res[..]).unwrap();
            json["links"].as_array().unwrap()
                .into_iter()
                .find_map(|link| {
                    if link["rel"].as_str().unwrap() == "self" && link["type"].as_str().unwrap() == "application/activity+json" {
                        Some(String::from(link["href"].as_str().unwrap()))
                    } else {
                        None
                    }
                }).unwrap()
        })
        .map_err(|e| format!("Error while fetchin WebFinger resource ({})", e))
}
