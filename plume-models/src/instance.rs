use chrono::NaiveDateTime;
use diesel::{self, ExpressionMethods, QueryDsl, RunQueryDsl};
use std::iter::Iterator;

use ap_url;
use plume_common::utils::md_to_html;
use safe_string::SafeString;
use schema::{instances, users};
use users::User;
use {Connection, Error, Result};

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
    pub long_description_html: SafeString,
    pub short_description_html: SafeString,
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
    pub default_license: String,
    pub long_description_html: String,
    pub short_description_html: String,
}

impl Instance {
    pub fn get_local(conn: &Connection) -> Result<Instance> {
        instances::table
            .filter(instances::local.eq(true))
            .limit(1)
            .load::<Instance>(conn)?
            .into_iter()
            .nth(0).ok_or(Error::NotFound)
    }

    pub fn get_remotes(conn: &Connection) -> Result<Vec<Instance>> {
        instances::table
            .filter(instances::local.eq(false))
            .load::<Instance>(conn)
            .map_err(Error::from)
    }

    pub fn page(conn: &Connection, (min, max): (i32, i32)) -> Result<Vec<Instance>> {
        instances::table
            .order(instances::public_domain.asc())
            .offset(min.into())
            .limit((max - min).into())
            .load::<Instance>(conn)
            .map_err(Error::from)
    }

    insert!(instances, NewInstance);
    get!(instances);
    find_by!(instances, find_by_domain, public_domain as &str);

    pub fn toggle_block(&self, conn: &Connection) -> Result<usize> {
        diesel::update(self)
            .set(instances::blocked.eq(!self.blocked))
            .execute(conn)
            .map_err(Error::from)
    }

    /// id: AP object id
    pub fn is_blocked(conn: &Connection, id: &str) -> Result<bool> {
        for block in instances::table
            .filter(instances::blocked.eq(true))
            .get_results::<Instance>(conn)?
        {
            if id.starts_with(&format!("https://{}/", block.public_domain)) {
                return Ok(true);
            }
        }

        Ok(false)
    }

    pub fn has_admin(&self, conn: &Connection) -> Result<bool> {
        users::table
            .filter(users::instance_id.eq(self.id))
            .filter(users::is_admin.eq(true))
            .load::<User>(conn)
            .map_err(Error::from)
            .map(|r| !r.is_empty())
    }

    pub fn main_admin(&self, conn: &Connection) -> Result<User> {
        users::table
            .filter(users::instance_id.eq(self.id))
            .filter(users::is_admin.eq(true))
            .limit(1)
            .get_result::<User>(conn)
            .map_err(Error::from)
    }

    pub fn compute_box(
        &self,
        prefix: &str,
        name: &str,
        box_name: &str,
    ) -> String {
        ap_url(&format!(
            "{instance}/{prefix}/{name}/{box_name}",
            instance = self.public_domain,
            prefix = prefix,
            name = name,
            box_name = box_name
        ))
    }

    pub fn update(
        &self,
        conn: &Connection,
        name: String,
        open_registrations: bool,
        short_description: SafeString,
        long_description: SafeString,
    ) -> Result<usize> {
        let (sd, _, _) = md_to_html(short_description.as_ref(), &self.public_domain);
        let (ld, _, _) = md_to_html(long_description.as_ref(), &self.public_domain);
        diesel::update(self)
            .set((
                instances::name.eq(name),
                instances::open_registrations.eq(open_registrations),
                instances::short_description.eq(short_description),
                instances::long_description.eq(long_description),
                instances::short_description_html.eq(sd),
                instances::long_description_html.eq(ld),
            ))
            .execute(conn)
            .map_err(Error::from)
    }

    pub fn count(conn: &Connection) -> Result<i64> {
        instances::table
            .count()
            .get_result(conn)
            .map_err(Error::from)
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use diesel::Connection;
    use tests::db;
    use Connection as Conn;

    pub(crate) fn fill_database(conn: &Conn) -> Vec<(NewInstance, Instance)> {
        vec![
            NewInstance {
                default_license: "WTFPL".to_string(),
                local: true,
                long_description: SafeString::new("This is my instance."),
                long_description_html: "<p>This is my instance</p>".to_string(),
                short_description: SafeString::new("My instance."),
                short_description_html: "<p>My instance</p>".to_string(),
                name: "My instance".to_string(),
                open_registrations: true,
                public_domain: "plu.me".to_string(),
            },
            NewInstance {
                default_license: "WTFPL".to_string(),
                local: false,
                long_description: SafeString::new("This is an instance."),
                long_description_html: "<p>This is an instance</p>".to_string(),
                short_description: SafeString::new("An instance."),
                short_description_html: "<p>An instance</p>".to_string(),
                name: "An instance".to_string(),
                open_registrations: true,
                public_domain: "1plu.me".to_string(),
            },
            NewInstance {
                default_license: "CC-0".to_string(),
                local: false,
                long_description: SafeString::new("This is the instance of someone."),
                long_description_html: "<p>This is the instance of someone</p>".to_string(),
                short_description: SafeString::new("Someone instance."),
                short_description_html: "<p>Someone instance</p>".to_string(),
                name: "Someone instance".to_string(),
                open_registrations: false,
                public_domain: "2plu.me".to_string(),
            },
            NewInstance {
                default_license: "CC-0-BY-SA".to_string(),
                local: false,
                long_description: SafeString::new("Good morning"),
                long_description_html: "<p>Good morning</p>".to_string(),
                short_description: SafeString::new("Hello"),
                short_description_html: "<p>Hello</p>".to_string(),
                name: "Nice day".to_string(),
                open_registrations: true,
                public_domain: "3plu.me".to_string(),
            },
        ].into_iter()
            .map(|inst| {
                (
                    inst.clone(),
                    Instance::find_by_domain(conn, &inst.public_domain)
                        .unwrap_or_else(|_| Instance::insert(conn, inst).unwrap()),
                )
            })
            .collect()
    }

    #[test]
    fn local_instance() {
        let conn = &db();
        conn.test_transaction::<_, (), _>(|| {
            let inserted = fill_database(conn)
                .into_iter()
                .map(|(inserted, _)| inserted)
                .find(|inst| inst.local)
                .unwrap();
            let res = Instance::get_local(conn).unwrap();

            part_eq!(
                res,
                inserted,
                [
                    default_license,
                    local,
                    long_description,
                    short_description,
                    name,
                    open_registrations,
                    public_domain
                ]
            );
            assert_eq!(res.long_description_html.get(), &inserted.long_description_html);
            assert_eq!(res.short_description_html.get(), &inserted.short_description_html);

            Ok(())
        });
    }

    #[test]
    fn remote_instance() {
        let conn = &db();
        conn.test_transaction::<_, (), _>(|| {
            let inserted = fill_database(conn);
            assert_eq!(Instance::count(conn).unwrap(), inserted.len() as i64);

            let res = Instance::get_remotes(conn).unwrap();
            assert_eq!(
                res.len(),
                inserted.iter().filter(|(inst, _)| !inst.local).count()
            );

            inserted
                .iter()
                .filter(|(newinst, _)| !newinst.local)
                .map(|(newinst, inst)| (newinst, res.iter().find(|res| res.id == inst.id).unwrap()))
                .for_each(|(newinst, inst)| {
                    part_eq!(
                        newinst,
                        inst,
                        [
                            default_license,
                            local,
                            long_description,
                            short_description,
                            name,
                            open_registrations,
                            public_domain
                        ]
                    );
                    assert_eq!(&newinst.long_description_html, inst.long_description_html.get());
                    assert_eq!(&newinst.short_description_html, inst.short_description_html.get());
                });

            let page = Instance::page(conn, (0, 2)).unwrap();
            assert_eq!(page.len(), 2);
            let page1 = &page[0];
            let page2 = &page[1];
            assert!(page1.public_domain <= page2.public_domain);

            let mut last_domaine: String = Instance::page(conn, (0, 1)).unwrap()[0].public_domain.clone();
            for i in 1..inserted.len() as i32 {
                let page = Instance::page(conn, (i, i + 1)).unwrap();
                assert_eq!(page.len(), 1);
                assert!(last_domaine <= page[0].public_domain);
                last_domaine = page[0].public_domain.clone();
            }

            Ok(())
        });
    }

    #[test]
    fn blocked() {
        let conn = &db();
        conn.test_transaction::<_, (), _>(|| {
            let inst_list = fill_database(conn);
            let inst = &inst_list[0].1;
            let inst_list = &inst_list[1..];

            let blocked = inst.blocked;
            inst.toggle_block(conn).unwrap();
            let inst = Instance::get(conn, inst.id).unwrap();
            assert_eq!(inst.blocked, !blocked);
            assert_eq!(
                inst_list
                    .iter()
                    .filter(
                        |(_, inst)| inst.blocked != Instance::get(conn, inst.id).unwrap().blocked
                    )
                    .count(),
                0
            );
            assert_eq!(
                Instance::is_blocked(conn, &format!("https://{}/something", inst.public_domain)).unwrap(),
                inst.blocked
            );
            assert_eq!(
                Instance::is_blocked(conn, &format!("https://{}a/something", inst.public_domain)).unwrap(),
                Instance::find_by_domain(conn, &format!("{}a", inst.public_domain))
                    .map(|inst| inst.blocked)
                    .unwrap_or(false)
            );

            inst.toggle_block(conn).unwrap();
            let inst = Instance::get(conn, inst.id).unwrap();
            assert_eq!(inst.blocked, blocked);
            assert_eq!(
                Instance::is_blocked(conn, &format!("https://{}/something", inst.public_domain)).unwrap(),
                inst.blocked
            );
            assert_eq!(
                Instance::is_blocked(conn, &format!("https://{}a/something", inst.public_domain)).unwrap(),
                Instance::find_by_domain(conn, &format!("{}a", inst.public_domain))
                    .map(|inst| inst.blocked)
                    .unwrap_or(false)
            );
            assert_eq!(
                inst_list
                    .iter()
                    .filter(
                        |(_, inst)| inst.blocked != Instance::get(conn, inst.id).unwrap().blocked
                    )
                    .count(),
                0
            );

            Ok(())
        });
    }

    #[test]
    fn update() {
        let conn = &db();
        conn.test_transaction::<_, (), _>(|| {
            let inst = &fill_database(conn)[0].1;

            inst.update(
                conn,
                "NewName".to_owned(),
                false,
                SafeString::new("[short](#link)"),
                SafeString::new("[long_description](/with_link)"),
            ).unwrap();
            let inst = Instance::get(conn, inst.id).unwrap();
            assert_eq!(inst.name, "NewName".to_owned());
            assert_eq!(inst.open_registrations, false);
            assert_eq!(
                inst.long_description.get(),
                "[long_description](/with_link)"
            );
            assert_eq!(
                inst.long_description_html,
                SafeString::new("<p><a href=\"/with_link\">long_description</a></p>\n")
            );
            assert_eq!(inst.short_description.get(), "[short](#link)");
            assert_eq!(
                inst.short_description_html,
                SafeString::new("<p><a href=\"#link\">short</a></p>\n")
            );

            Ok(())
        });
    }
}
