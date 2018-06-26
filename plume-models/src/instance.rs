use chrono::NaiveDateTime;
use diesel::{self, QueryDsl, RunQueryDsl, ExpressionMethods, PgConnection};
use std::iter::Iterator;

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
    pub creation_date: NaiveDateTime    
}

#[derive(Insertable)]
#[table_name = "instances"]
pub struct NewInstance {
    pub public_domain: String,
    pub name: String,
    pub local: bool
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

    pub fn local_id(conn: &PgConnection) -> i32 {
        Instance::get_local(conn).unwrap().id
    }

    insert!(instances, NewInstance);
    get!(instances);
    find_by!(instances, find_by_domain, public_domain as String);

    pub fn block(&self) {
        unimplemented!()
    }

    pub fn has_admin(&self, conn: &PgConnection) -> bool {
        users::table.filter(users::instance_id.eq(self.id))
            .filter(users::is_admin.eq(true))
            .load::<User>(conn)
            .expect("Couldn't load admins")
            .len() > 0
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
}
