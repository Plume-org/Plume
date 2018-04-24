use diesel::PgConnection;
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
