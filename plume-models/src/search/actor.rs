use super::Searcher;
use crate::{db_conn::DbPool, posts::PostEvent, ACTOR_SYS, POST_CHAN};
use riker::actors::{Actor, ActorFactoryArgs, ActorRefFactory, Context, Sender, Subscribe, Tell};
use std::sync::Arc;
use std::thread::sleep;
use std::time::Duration;
use tracing::error;

pub struct SearchActor {
    searcher: Arc<Searcher>,
    conn: DbPool,
}

impl SearchActor {
    pub fn init(searcher: Arc<Searcher>, conn: DbPool) {
        let actor = ACTOR_SYS
            .actor_of_args::<SearchActor, _>("search", (searcher, conn))
            .expect("Failed to initialize searcher actor");

        POST_CHAN.tell(
            Subscribe {
                actor: Box::new(actor),
                topic: "*".into(),
            },
            None,
        )
    }
}

impl Actor for SearchActor {
    type Msg = PostEvent;

    fn recv(&mut self, _ctx: &Context<Self::Msg>, msg: Self::Msg, _sender: Sender) {
        use PostEvent::*;

        // Wait for transaction commited
        sleep(Duration::from_millis(500));

        match msg {
            PostPublished(post) => {
                match self.conn.get() {
                    Ok(mut conn) => {
                        self.searcher
                            .add_document(&mut conn, &post)
                            .unwrap_or_else(|e| error!("{:?}", e));
                    }
                    _ => {
                        error!("Failed to get database connection");
                    }
                }
            }
            PostUpdated(post) => {
                match self.conn.get() {
                    Ok(mut conn) => {
                        self.searcher
                            .update_document(&mut conn, &post)
                            .unwrap_or_else(|e| error!("{:?}", e));
                    }
                    _ => {
                        error!("Failed to get database connection");
                    }
                }
            }
            PostDeleted(post) => self.searcher.delete_document(&post),
        }
    }
}

impl ActorFactoryArgs<(Arc<Searcher>, DbPool)> for SearchActor {
    fn create_args((searcher, conn): (Arc<Searcher>, DbPool)) -> Self {
        Self { searcher, conn }
    }
}

#[cfg(test)]
mod tests {
    use crate::diesel::Connection;
    use crate::{
        blog_authors::{BlogAuthor, NewBlogAuthor},
        blogs::{Blog, NewBlog},
        db_conn::{DbPool, PragmaForeignKey},
        instance::{Instance, NewInstance},
        post_authors::{NewPostAuthor, PostAuthor},
        posts::{NewPost, Post},
        safe_string::SafeString,
        search::{actor::SearchActor, tests::get_searcher, Query},
        users::{NewUser, User},
        Connection as Conn, CONFIG,
    };
    use diesel::r2d2::ConnectionManager;
    use plume_common::utils::random_hex;
    use std::str::FromStr;
    use std::sync::Arc;
    use std::thread::sleep;
    use std::time::Duration;

    #[test]
    fn post_updated() {
        // Need to commit so that searcher on another thread retrieve records.
        // So, build DbPool instead of using DB_POOL for testing.
        let manager = ConnectionManager::<Conn>::new(CONFIG.database_url.as_str());
        let db_pool = DbPool::builder()
            .connection_customizer(Box::new(PragmaForeignKey))
            .build(manager)
            .unwrap();

        let searcher = Arc::new(get_searcher(&CONFIG.search_tokenizers));
        SearchActor::init(searcher.clone(), db_pool.clone());
        let conn = db_pool.get().unwrap();

        let title = random_hex()[..8].to_owned();
        let (_instance, _user, blog) = fill_database(&conn);
        let author = &blog.list_authors(&conn).unwrap()[0];

        let post = Post::insert(
            &conn,
            NewPost {
                blog_id: blog.id,
                slug: title.clone(),
                title: title.clone(),
                content: SafeString::new(""),
                published: true,
                license: "CC-BY-SA".to_owned(),
                ap_url: "".to_owned(),
                creation_date: None,
                subtitle: "".to_owned(),
                source: "".to_owned(),
                cover_id: None,
            },
        )
        .unwrap();
        PostAuthor::insert(
            &conn,
            NewPostAuthor {
                post_id: post.id,
                author_id: author.id,
            },
        )
        .unwrap();
        let post_id = post.id;

        // Wait for searcher on another thread add document asynchronously
        sleep(Duration::from_millis(700));
        searcher.commit();
        assert_eq!(
            searcher.search_document(&conn, Query::from_str(&title).unwrap(), (0, 1))[0].id,
            post_id
        );
    }

    fn fill_database(conn: &Conn) -> (Instance, User, Blog) {
        conn.transaction::<(Instance, User, Blog), diesel::result::Error, _>(|| {
            let instance = Instance::insert(
                conn,
                NewInstance {
                    default_license: "CC-0-BY-SA".to_string(),
                    local: false,
                    long_description: SafeString::new("Good morning"),
                    long_description_html: "<p>Good morning</p>".to_string(),
                    short_description: SafeString::new("Hello"),
                    short_description_html: "<p>Hello</p>".to_string(),
                    name: random_hex(),
                    open_registrations: true,
                    public_domain: random_hex(),
                },
            )
            .unwrap();
            let user = User::insert(
                conn,
                NewUser {
                    username: random_hex(),
                    display_name: random_hex(),
                    outbox_url: random_hex(),
                    inbox_url: random_hex(),
                    summary: "".to_string(),
                    email: None,
                    hashed_password: None,
                    instance_id: instance.id,
                    ap_url: random_hex(),
                    private_key: None,
                    public_key: "".to_string(),
                    shared_inbox_url: None,
                    followers_endpoint: random_hex(),
                    avatar_id: None,
                    summary_html: SafeString::new(""),
                    role: 0,
                    fqn: random_hex(),
                },
            )
            .unwrap();
            let blog = NewBlog {
                instance_id: instance.id,
                actor_id: random_hex(),
                ap_url: random_hex(),
                inbox_url: random_hex(),
                outbox_url: random_hex(),
                ..Default::default()
            };
            let blog = Blog::insert(conn, blog).unwrap();
            BlogAuthor::insert(
                conn,
                NewBlogAuthor {
                    blog_id: blog.id,
                    author_id: user.id,
                    is_owner: true,
                },
            )
            .unwrap();

            Ok((instance, user, blog))
        })
        .unwrap()
    }
}
