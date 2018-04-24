use diesel::PgConnection;
use serde_json;

pub trait Webfinger {
    fn webfinger_subject(&self, conn: &PgConnection) -> String;
    fn webfinger_aliases(&self, conn: &PgConnection) -> Vec<String>;
    fn webfinger_links(&self, conn: &PgConnection) -> Vec<Vec<(String, String)>>;

    fn webfinger_json(&self, conn: &PgConnection) -> serde_json::Value {
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
        })
    }

    fn webfinger_xml(&self, conn: &PgConnection) -> String {
        format!(r#"
            <?xml version="1.0"?>
            <XRD xmlns="http://docs.oasis-open.org/ns/xri/xrd-1.0">
            <Subject>{subject}</Subject>
            {aliases}
            {links}
            </XRD>
            "#,
            subject = self.webfinger_subject(conn),
            aliases = self.webfinger_aliases(conn).into_iter().map(|a| {
                format!("<Alias>{a}</Alias>", a = a)
            }).collect::<Vec<String>>().join("\n"),
            links = self.webfinger_links(conn).into_iter().map(|l| {
                format!("<Link {} />", l.into_iter().map(|prop| {
                    format!("{}=\"{}\"", prop.0, prop.1)
                }).collect::<Vec<String>>().join(" "))
            }).collect::<Vec<String>>().join("\n")
        )
    }

    fn webfinger(&self, format: &'static str, conn: &PgConnection) -> String {
        match format {
            "json" => self.webfinger_json(conn).to_string(),
            _ => self.webfinger_xml(conn)
        }
    }
}
