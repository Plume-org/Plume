use diesel;
use diesel::{ QueryDsl, RunQueryDsl, ExpressionMethods, PgConnection };
use std::iter::Iterator;
use schema::{instances, users};
use models::user::User;

#[derive(Identifiable, Queryable)]
pub struct Instance {
    pub id: i32,
    pub local_domain: String,
    pub public_domain: String,
    pub name: String,
    pub local: bool,
    pub blocked: bool
}

#[derive(Insertable)]
#[table_name = "instances"]
pub struct NewInstance {
    pub local_domain: String,
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

    pub fn insert<'a>(conn: &PgConnection, loc_dom: String, pub_dom: String, name: String, local: bool) -> Instance {
        diesel::insert_into(instances::table)
            .values(NewInstance {
                local_domain: loc_dom,
                public_domain: pub_dom,
                name: name,
                local: local
            })
            .get_result(conn)
            .expect("Error saving new instance")
    }

    pub fn get(conn: &PgConnection, id: i32) -> Option<Instance> {
        instances::table.filter(instances::id.eq(id))
            .limit(1)
            .load::<Instance>(conn)
            .expect("Error loading local instance infos")
            .into_iter().nth(0)
    }

    pub fn block(&self) {}

    pub fn has_admin(&self, conn: &PgConnection) -> bool {
        users::table.filter(users::instance_id.eq(self.id))
            .filter(users::is_admin.eq(true))
            .load::<User>(conn)
            .expect("Couldn't load admins")
            .len() > 0
    }
}
