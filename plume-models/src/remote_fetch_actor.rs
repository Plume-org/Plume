use crate::{
    db_conn::{DbConn, DbPool},
    follows,
    posts::Post,
    users::{User, UserEvent},
    ACTOR_SYS, CONFIG, USER_CHAN,
};
use activitystreams::{
    activity::{AsActivityObjectExt, Create},
    base::AnyBase,
    object::kind::ArticleType,
};
use plume_common::activity_pub::{inbox::FromId, LicensedArticle};
use riker::actors::{Actor, ActorFactoryArgs, ActorRefFactory, Context, Sender, Subscribe, Tell};
use std::sync::Arc;
use tracing::{error, info, warn};
use futures::executor::ThreadPool;

pub struct RemoteFetchActor {
    conn: DbPool,
    pool: ThreadPool,
}

impl RemoteFetchActor {

    pub fn init(conn: DbPool) {
        let actor = ACTOR_SYS
            .actor_of_args::<RemoteFetchActor, _>("remote-fetch", conn)
            .expect("Failed to initialize remote fetch actor");

        USER_CHAN.tell(
            Subscribe {
                actor: Box::new(actor),
                topic: "*".into(),
            },
            None,
        )
    }
}

impl Actor for RemoteFetchActor {
    type Msg = UserEvent;

    fn recv(&mut self, _ctx: &Context<Self::Msg>, msg: Self::Msg, _sender: Sender) {
        use UserEvent::*;

        match msg {
            RemoteUserFound(user) => {
                match self.conn.get() {
                    Ok(conn) => {
                        let mut conn = DbConn(conn);
                        if user
                            .get_instance(&mut conn)
                            .map_or(false, |instance| instance.blocked)
                        {
                            return;
                        }
                        // Don't call these functions in parallel
                        // for the case database connections limit is too small
                        self.pool.spawn_ok(fetch_and_cache_articles(user.clone(), conn));
                    }
                    _ => error!("Failed to get database connection"),
                };

                match self.conn.get() {
                    Ok(conn) => {
                        // Don't call these functions in parallel
                        // for the case database connections limit is too small
                        self.pool.spawn_ok(fetch_and_cache_followers(user.clone(), DbConn(conn)));
                    }
                    _ => error!("Failed to get database connection"),
                };

                if user.needs_update() {
                    match self.conn.get() {
                        Ok(conn) => fetch_and_cache_user(&user, DbConn(conn)),
                        _ => error!("Failed to get database connection"),
                    };
                }
            }
        }
    }
}

impl ActorFactoryArgs<DbPool> for RemoteFetchActor {
    fn create_args(conn: DbPool) -> Self {
        let pool: ThreadPool = ThreadPool::new().unwrap();
        Self { conn, pool }
    }
}

async fn fetch_and_cache_articles(user: Arc<User>, mut conn: DbConn) {
    let create_acts = user.fetch_outbox::<Create>();
    match create_acts {
        Ok(create_acts) => {
            for create_act in create_acts {
                match create_act.object_unchecked().as_single_base().map(|base| {
                    let any_base = AnyBase::from_base(base.clone()); // FIXME: Don't clone()
                    any_base.extend::<LicensedArticle, ArticleType>()
                }) {
                    Some(Ok(Some(article))) => {
                        Post::from_activity(&mut conn, article).await
                            .expect("Article from remote user couldn't be saved");
                        info!("Fetched article from remote user");
                    }
                    Some(Err(e)) => warn!("Error while fetching articles in background: {:?}", e),
                    _ => warn!("Error while fetching articles in background"),
                }
            }
        }
        Err(err) => {
            error!("Failed to fetch outboxes: {:?}", err);
        }
    }
}

async fn fetch_and_cache_followers(user: Arc<User>, mut conn: DbConn) {
    let follower_ids = user.fetch_followers_ids();
    match follower_ids {
        Ok(user_ids) => {
            for user_id in user_ids {
                let follower = User::from_id(&mut conn, &user_id, None, CONFIG.proxy()).await;
                match follower {
                    Ok(follower) => {
                        let inserted = follows::Follow::insert(
                            &mut conn,
                            follows::NewFollow {
                                follower_id: follower.id,
                                following_id: user.id,
                                ap_url: String::new(),
                            },
                        );
                        if inserted.is_err() {
                            error!("Couldn't save follower for remote user: {:?}", user_id);
                        }
                    }
                    Err(err) => {
                        error!("Couldn't fetch follower: {:?}", err);
                    }
                }
            }
        }
        Err(err) => {
            error!("Failed to fetch follower: {:?}", err);
        }
    }
}

fn fetch_and_cache_user(user: &Arc<User>, mut conn: DbConn) {
    if user.refetch(&mut conn).is_err() {
        error!("Couldn't update user info: {:?}", user);
    }
}
