use chrono::NaiveDateTime;
use diesel::{self, QueryDsl, RunQueryDsl, ExpressionMethods, PgConnection};
use serde_json;
use std::iter::Iterator;

use activity_pub::inbox::Inbox;
use models::users::User;
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

    pub fn insert<'a>(conn: &PgConnection, pub_dom: String, name: String, local: bool) -> Instance {
        diesel::insert_into(instances::table)
            .values(NewInstance {
                public_domain: pub_dom,
                name: name,
                local: local
            })
            .get_result(conn)
            .expect("Error saving new instance")
    }

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
}

impl Inbox for Instance {
    fn received(&self, conn: &PgConnection, act: serde_json::Value) {
        self.save(conn, act.clone()).expect("Shared Inbox: Couldn't save activity");

        // TODO: add to stream, or whatever needs to be done
    }
}
