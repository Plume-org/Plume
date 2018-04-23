use diesel;
use diesel::{QueryDsl, RunQueryDsl, ExpressionMethods, PgConnection};
use schema::blogs;
use activity_pub::Actor;
use models::instance::Instance;

#[derive(Queryable, Identifiable)]
pub struct Blog {
    pub id: i32,
    pub actor_id: String,
    pub title: String,
    pub summary: String,
    pub outbox_url: String,
    pub inbox_url: String,
    pub instance_id: i32
}

#[derive(Insertable)]
#[table_name = "blogs"]
pub struct NewBlog {
    pub actor_id: String,
    pub title: String,
    pub summary: String,
    pub outbox_url: String,
    pub inbox_url: String,
    pub instance_id: i32
}

impl Blog {
    pub fn insert (conn: &PgConnection, new: NewBlog) -> Blog {
        diesel::insert_into(blogs::table)
            .values(new)
            .get_result(conn)
            .expect("Error saving new blog")
    }

    pub fn compute_outbox(blog: String, hostname: String) -> String {
        format!("https://{}/~/{}/outbox", hostname, blog)
    }

    pub fn compute_inbox(blog: String, hostname: String) -> String {
        format!("https://{}/~/{}/inbox", hostname, blog)
    }

    pub fn get(conn: &PgConnection, id: i32) -> Option<Blog> {
        blogs::table.filter(blogs::id.eq(id))
            .limit(1)
            .load::<Blog>(conn)
            .expect("Error loading blog by id")
            .into_iter().nth(0)
    }

    pub fn find_by_actor_id(conn: &PgConnection, username: String) -> Option<Blog> {
        blogs::table.filter(blogs::actor_id.eq(username))
            .limit(1)
            .load::<Blog>(conn)
            .expect("Error loading blog by email")
            .into_iter().nth(0)
    }
}

impl Actor for Blog {
    fn get_box_prefix() -> &'static str {
        "~"
    }

    fn get_actor_id(&self) -> String {
        self.actor_id.to_string()
    }

    fn get_instance(&self, conn: &PgConnection) -> Instance {
        Instance::get(conn, self.instance_id).unwrap()
    }
}
