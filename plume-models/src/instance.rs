use chrono::NaiveDateTime;
use diesel::{self, QueryDsl, RunQueryDsl, ExpressionMethods};
use std::iter::Iterator;

use plume_common::utils::md_to_html;
use Connection;
use safe_string::SafeString;
use ap_url;
use users::User;
use schema::{instances, users};

#[derive(Clone, Identifiable, Queryable, Serialize)]
pub struct Instance {
    pub id: i32,
    pub public_domain: String,
    pub name: String,
    pub local: bool,
    pub blocked: bool,
    pub creation_date: NaiveDateTime,
    pub open_registrations: bool,
    pub short_description: SafeString,
    pub long_description: SafeString,
    pub default_license : String,
    pub long_description_html: String,
    pub short_description_html: String
}

#[derive(Clone, Insertable)]
#[table_name = "instances"]
pub struct NewInstance {
    pub public_domain: String,
    pub name: String,
    pub local: bool,
    pub open_registrations: bool,
    pub short_description: SafeString,
    pub long_description: SafeString,
    pub default_license : String,
    pub long_description_html: String,
    pub short_description_html: String
}

impl Instance {
    pub fn get_local(conn: &Connection) -> Option<Instance> {
        instances::table.filter(instances::local.eq(true))
            .limit(1)
            .load::<Instance>(conn)
            .expect("Instance::get_local: loading error")
            .into_iter().nth(0)
    }

    pub fn get_remotes(conn: &Connection) -> Vec<Instance> {
        instances::table.filter(instances::local.eq(false))
            .load::<Instance>(conn)
            .expect("Instance::get_remotes: loading error")
    }

    pub fn page(conn: &Connection, (min, max): (i32, i32)) -> Vec<Instance> {
        instances::table.order(instances::public_domain.asc())
            .offset(min.into())
            .limit((max - min).into())
            .load::<Instance>(conn)
            .expect("Instance::page: loading error")
    }

    pub fn local_id(conn: &Connection) -> i32 {
        Instance::get_local(conn).expect("Instance::local_id: local instance not found error").id
    }

    insert!(instances, NewInstance);
    get!(instances);
    find_by!(instances, find_by_domain, public_domain as String);

    pub fn toggle_block(&self, conn: &Connection) {
        diesel::update(self)
            .set(instances::blocked.eq(!self.blocked))
            .execute(conn)
            .expect("Instance::toggle_block: update error");
    }

    /// id: AP object id
    pub fn is_blocked(conn: &Connection, id: String) -> bool {
        for block in instances::table.filter(instances::blocked.eq(true))
            .get_results::<Instance>(conn)
            .expect("Instance::is_blocked: loading error") {
            if id.starts_with(format!("https://{}/", block.public_domain).as_str()) {
                return true;
            }
        }

        false
    }

    pub fn has_admin(&self, conn: &Connection) -> bool {
        users::table.filter(users::instance_id.eq(self.id))
            .filter(users::is_admin.eq(true))
            .load::<User>(conn)
            .expect("Instance::has_admin: loading error")
            .len() > 0
    }

    pub fn main_admin(&self, conn: &Connection) -> User {
        users::table.filter(users::instance_id.eq(self.id))
            .filter(users::is_admin.eq(true))
            .limit(1)
            .get_result::<User>(conn)
            .expect("Instance::main_admin: loading error")
    }

    pub fn compute_box(&self, prefix: &'static str, name: String, box_name: &'static str) -> String {
        ap_url(format!(
            "{instance}/{prefix}/{name}/{box_name}",
            instance = self.public_domain,
            prefix = prefix,
            name = name,
            box_name = box_name
        ))
    }

    pub fn update(&self, conn: &Connection, name: String, open_registrations: bool, short_description: SafeString, long_description: SafeString) {
        let (sd, _, _) = md_to_html(short_description.as_ref());
        let (ld, _, _) = md_to_html(long_description.as_ref());
        diesel::update(self)
            .set((
                instances::name.eq(name),
                instances::open_registrations.eq(open_registrations),
                instances::short_description.eq(short_description),
                instances::long_description.eq(long_description),
                instances::short_description_html.eq(sd),
                instances::long_description_html.eq(ld)
            )).execute(conn)
            .expect("Instance::update: update error");
    }

    pub fn count(conn: &Connection) -> i64 {
        instances::table.count().get_result(conn).expect("Instance::count: counting error")
    }
}


#[cfg(test)]
mod tests {
    use diesel::Connection;
    use tests::db;
    use super::*;

    #[test]
    fn local_instance() {
        let conn = &db();
        conn.test_transaction::<_, (), _>(|| {
            let inserted = NewInstance {
                default_license: "WTFPL".to_string(),
                local: true,
                long_description: SafeString::new("This is my instance."),
                long_description_html: "<p>This is my instance</p>".to_string(),
                short_description: SafeString::new("My instance."),
                short_description_html: "<p>My instance</p>".to_string(),
                name: "My instance".to_string(),
                open_registrations: true,
                public_domain: "plu.me".to_string(),
            };
            Instance::insert(conn, inserted.clone());
            let res = Instance::get_local(conn).unwrap();

            part_eq!(res, inserted, [
                     default_license,
                     local,
                     long_description,
                     long_description_html,
                     short_description,
                     short_description_html,
                     name,
                     open_registrations,
                     public_domain
                     ]);
            assert_eq!(Instance::local_id(conn), res.id);
            Ok(())
        });
    }

    #[test]
    fn remote_instance() {
        let conn = &db();
        conn.test_transaction::<_, (), _>(|| {
            let inserted1 = NewInstance {
                default_license: "WTFPL".to_string(),
                local: false,
                long_description: SafeString::new("This is my instance."),
                long_description_html: "<p>This is my instance</p>".to_string(),
                short_description: SafeString::new("My instance."),
                short_description_html: "<p>My instance</p>".to_string(),
                name: "My instance".to_string(),
                open_registrations: true,
                public_domain: "1plu.me".to_string(),
            };
            let inserted2 = NewInstance {
                default_license: "CC-0".to_string(),
                local: false,
                long_description: SafeString::new("This is the instance of someone."),
                long_description_html: "<p>This is the instance of someone</p>".to_string(),
                short_description: SafeString::new("Someone instance."),
                short_description_html: "<p>Someone instance</p>".to_string(),
                name: "Someone instance".to_string(),
                open_registrations: false,
                public_domain: "2plu.me".to_string(),
            };
            let inserted3 = NewInstance {
                default_license: "CC-0-BY-SA".to_string(),
                local: false,
                long_description: SafeString::new("Good morning"),
                long_description_html: "<p>Good morning</p>".to_string(),
                short_description: SafeString::new("Hello"),
                short_description_html: "<p>Hello</p>".to_string(),
                name: "Nice day".to_string(),
                open_registrations: true,
                public_domain: "3plu.me".to_string(),
            };
            Instance::insert(conn, inserted1.clone());
            Instance::insert(conn, inserted2.clone());
            Instance::insert(conn, inserted3.clone());
            assert_eq!(Instance::count(conn), 3);
            let res = Instance::get_remotes(conn);
            assert_eq!(res.len(), 3);

            part_eq!(res[0], inserted1, [
                     default_license,
                     local,
                     long_description,
                     long_description_html,
                     short_description,
                     short_description_html,
                     name,
                     open_registrations,
                     public_domain
                     ]);

            part_eq!(res[1], inserted2, [
                     default_license,
                     local,
                     long_description,
                     long_description_html,
                     short_description,
                     short_description_html,
                     name,
                     open_registrations,
                     public_domain
                     ]);

            part_eq!(res[2], inserted3, [
                     default_license,
                     local,
                     long_description,
                     long_description_html,
                     short_description,
                     short_description_html,
                     name,
                     open_registrations,
                     public_domain
                     ]);

            let mut page = Instance::page(conn, (1, 2));
            assert_eq!(page.len(), 1);
            let page_res = page.remove(0);

            part_eq!(page_res, res[1], [
                id,
                public_domain,
                name,
                local,
                blocked,
                creation_date,
                open_registrations,
                short_description,
                long_description,
                default_license,
                long_description_html,
                short_description_html
            ]);

            Ok(())
        });
    }

    #[test]
    fn blocked() {
        let conn = &db();
        conn.test_transaction::<_, (), _>(|| {
            let inst = Instance::insert(conn, NewInstance {
                default_license: "WTFPL".to_string(),
                local: false,
                long_description: SafeString::new("This is my instance."),
                long_description_html: "<p>This is my instance</p>".to_string(),
                short_description: SafeString::new("My instance."),
                short_description_html: "<p>My instance</p>".to_string(),
                name: "My instance".to_string(),
                open_registrations: true,
                public_domain: "1plu.me".to_string(),
            });

            assert!(!inst.blocked);
            inst.toggle_block(conn);
            let inst = Instance::get(conn, inst.id).unwrap();
            assert!(inst.blocked);
            assert!(Instance::is_blocked(conn, "https://1plu.me/something".to_owned()));
            assert!(!Instance::is_blocked(conn, "https://1plu.mea/something".to_owned()));
            assert!(!Instance::is_blocked(conn, "https://other/something".to_owned()));

            inst.toggle_block(conn);
            let inst = Instance::get(conn, inst.id).unwrap();
            assert!(!inst.blocked);
            assert!(!Instance::is_blocked(conn, "https://1plu.me/something".to_owned()));
            assert!(!Instance::is_blocked(conn, "https://1plu.mea/something".to_owned()));
            assert!(!Instance::is_blocked(conn, "https://other/something".to_owned()));

            Ok(())
        });
    }

    #[test]
    fn update() {
        let conn = &db();
        conn.test_transaction::<_, (), _>(|| {
            let inst = Instance::insert(conn, NewInstance {
                default_license: "WTFPL".to_string(),
                local: true,
                long_description: SafeString::new("This is my instance."),
                long_description_html: "<p>This is my instance</p>".to_string(),
                short_description: SafeString::new("My instance."),
                short_description_html: "<p>My instance</p>".to_string(),
                name: "My instance".to_string(),
                open_registrations: true,
                public_domain: "plu.me".to_string(),
            });

            inst.update(conn, "NewName".to_owned(), false, SafeString::new("[short](#link)"), SafeString::new("[long_description](/with_link)"));
            let inst = Instance::get(conn, inst.id).unwrap();
            assert_eq!(inst.name, "NewName".to_owned());
            assert_eq!(inst.open_registrations, false);
            assert_eq!(inst.long_description.get(), "[long_description](/with_link)");
            assert_eq!(inst.long_description_html, "<p><a href=\"/with_link\">long_description</a></p>\n");
            assert_eq!(inst.short_description.get(), "[short](#link)");
            assert_eq!(inst.short_description_html, "<p><a href=\"#link\">short</a></p>\n");


            Ok(())
        });
    }
}
