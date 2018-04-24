use diesel;
use diesel::{QueryDsl, RunQueryDsl, ExpressionMethods, PgConnection};
use schema::blogs;
use activity_pub::{Actor, ActorType};
use models::instance::Instance;
use activity_pub::webfinger::*;

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

    pub fn update_boxes(&self, conn: &PgConnection) {
        if self.outbox_url.len() == 0 {
            diesel::update(self)
                .set(blogs::outbox_url.eq(self.compute_outbox(conn)))
                .get_result::<Blog>(conn).expect("Couldn't update outbox URL");
        }

        if self.inbox_url.len() == 0 {
            diesel::update(self)
                .set(blogs::inbox_url.eq(self.compute_inbox(conn)))
                .get_result::<Blog>(conn).expect("Couldn't update inbox URL");
        }
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

    fn get_actor_type () -> ActorType {
        ActorType::Blog
    }
}

impl Webfinger for Blog {
    fn webfinger_subject(&self, conn: &PgConnection) -> String {
        format!("acct:{}@{}", self.actor_id, self.get_instance(conn).public_domain)
    }
    fn webfinger_aliases(&self, conn: &PgConnection) -> Vec<String> {
        vec![self.compute_id(conn)]
    }
    fn webfinger_links(&self, conn: &PgConnection) -> Vec<Vec<(String, String)>> {
        vec![
            vec![
                (String::from("rel"), String::from("http://webfinger.net/rel/profile-page")),
                (String::from("href"), self.compute_id(conn))
            ],
            vec![
                (String::from("rel"), String::from("http://schemas.google.com/g/2010#updates-from")),
                (String::from("type"), String::from("application/atom+xml")),
                (String::from("href"), self.compute_box(conn, "feed.atom"))
            ],
            vec![
                (String::from("rel"), String::from("self")),
                (String::from("type"), String::from("application/activity+json")),
                (String::from("href"), self.compute_id(conn))
            ]
        ]
    }
}

impl NewBlog {
    pub fn new_local(
        actor_id: String,
        title: String,
        summary: String,
        instance_id: i32
    ) -> NewBlog {
        NewBlog {
            actor_id: actor_id,
            title: title,
            summary: summary,
            outbox_url: String::from(""),
            inbox_url: String::from(""),
            instance_id: instance_id
        }
    }
}
