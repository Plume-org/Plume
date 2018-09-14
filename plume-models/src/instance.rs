use chrono::NaiveDateTime;
use diesel::{self, QueryDsl, RunQueryDsl, ExpressionMethods, PgConnection};
use std::iter::Iterator;

use plume_common::utils::md_to_html;
use safe_string::SafeString;
use ap_url;
use users::User;
use schema::{instances, users};

#[derive(Identifiable, Queryable, Serialize)]
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

#[derive(Insertable)]
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
    pub fn get_local(conn: &PgConnection) -> Option<Instance> {
        instances::table.filter(instances::local.eq(true))
            .limit(1)
            .load::<Instance>(conn)
            .expect("Error loading local instance infos")
            .into_iter().nth(0)
    }

    pub fn get_remotes(conn: &PgConnection) -> Vec<Instance> {
        instances::table.filter(instances::local.eq(false))
            .load::<Instance>(conn)
            .expect("Error loading remote instances infos")
    }

    pub fn page(conn: &PgConnection, (min, max): (i32, i32)) -> Vec<Instance> {
        instances::table.order(instances::public_domain.asc())
            .offset(min.into())
            .limit((max - min).into())
            .load::<Instance>(conn)
            .expect("Error loading a page of instances")
    }

    pub fn local_id(conn: &PgConnection) -> i32 {
        Instance::get_local(conn).unwrap().id
    }

    insert!(instances, NewInstance);
    get!(instances);
    find_by!(instances, find_by_domain, public_domain as String);

    pub fn toggle_block(&self, conn: &PgConnection) {
        diesel::update(self)
            .set(instances::blocked.eq(!self.blocked))
            .get_result::<Instance>(conn)
            .expect("Couldn't block/unblock instance");
    }

    /// id: AP object id
    pub fn is_blocked(conn: &PgConnection, id: String) -> bool {
        for block in instances::table.filter(instances::blocked.eq(true))
            .get_results::<Instance>(conn)
            .expect("Error listing blocked instances") {
            if id.starts_with(format!("https://{}", block.public_domain).as_str()) {
                return true;
            }
        }

        false
    }

    pub fn has_admin(&self, conn: &PgConnection) -> bool {
        users::table.filter(users::instance_id.eq(self.id))
            .filter(users::is_admin.eq(true))
            .load::<User>(conn)
            .expect("Couldn't load admins")
            .len() > 0
    }

    pub fn main_admin(&self, conn: &PgConnection) -> User {
        users::table.filter(users::instance_id.eq(self.id))
            .filter(users::is_admin.eq(true))
            .limit(1)
            .get_result::<User>(conn)
            .expect("Couldn't load admins")
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

    pub fn update(&self, conn: &PgConnection, name: String, open_registrations: bool, short_description: SafeString, long_description: SafeString) -> Instance {
        let (sd, _) = md_to_html(short_description.as_ref());
        let (ld, _) = md_to_html(long_description.as_ref());
        diesel::update(self)
            .set((
                instances::name.eq(name),
                instances::open_registrations.eq(open_registrations),
                instances::short_description.eq(short_description),
                instances::long_description.eq(long_description),
                instances::short_description_html.eq(sd),
                instances::long_description_html.eq(ld)
            )).get_result::<Instance>(conn)
            .expect("Couldn't update instance")
    }

    pub fn count(conn: &PgConnection) -> i64 {
        instances::table.count().get_result(conn).expect("Couldn't count instances")
    }
}
