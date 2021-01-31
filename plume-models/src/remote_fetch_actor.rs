use crate::{
    db_conn::{DbConn, DbPool},
    follows,
    posts::{LicensedArticle, Post},
    users::{User, UserEvent},
    ACTOR_SYS, CONFIG, USER_CHAN,
};
use activitypub::activity::Create;
use plume_common::activity_pub::inbox::FromId;
use riker::actors::{Actor, ActorFactoryArgs, ActorRefFactory, Context, Sender, Subscribe, Tell};
use std::sync::Arc;
use tracing::{error, info, warn};

pub struct RemoteFetchActor {
    conn: DbPool,
}

impl RemoteFetchActor {
    pub fn init(conn: DbPool) {
        ACTOR_SYS
            .actor_of_args::<RemoteFetchActor, _>("remote-fetch", conn)
            .expect("Failed to initialize remote fetch actor");
    }
}

impl Actor for RemoteFetchActor {
    type Msg = UserEvent;

    fn pre_start(&mut self, ctx: &Context<Self::Msg>) {
        USER_CHAN.tell(
            Subscribe {
                actor: Box::new(ctx.myself()),
                topic: "*".into(),
            },
            None,
        )
    }

    fn recv(&mut self, _ctx: &Context<Self::Msg>, msg: Self::Msg, _sender: Sender) {
        use UserEvent::*;

        match msg {
            RemoteUserFound(user) => match self.conn.get() {
                Ok(conn) => {
                    let conn = DbConn(conn);
                    // Don't call these functions in parallel
                    // for the case database connections limit is too small
                    fetch_and_cache_articles(&user, &conn);
                    fetch_and_cache_followers(&user, &conn);
                    if user.needs_update() {
                        fetch_and_cache_user(&user, &conn);
                    }
                }
                _ => {
                    error!("Failed to get database connection");
                }
            },
        }
    }
}

impl ActorFactoryArgs<DbPool> for RemoteFetchActor {
    fn create_args(conn: DbPool) -> Self {
        Self { conn }
    }
}

fn fetch_and_cache_articles(user: &Arc<User>, conn: &DbConn) {
    let create_acts = user.fetch_outbox::<Create>();
    match create_acts {
        Ok(create_acts) => {
            for create_act in create_acts {
                match create_act.create_props.object_object::<LicensedArticle>() {
                    Ok(article) => {
                        Post::from_activity(conn, article)
                            .expect("Article from remote user couldn't be saved");
                        info!("Fetched article from remote user");
                    }
                    Err(e) => warn!("Error while fetching articles in background: {:?}", e),
                }
            }
        }
        Err(err) => {
            error!("Failed to fetch outboxes: {:?}", err);
        }
    }
}

fn fetch_and_cache_followers(user: &Arc<User>, conn: &DbConn) {
    let follower_ids = user.fetch_followers_ids();
    match follower_ids {
        Ok(user_ids) => {
            for user_id in user_ids {
                let follower = User::from_id(conn, &user_id, None, CONFIG.proxy());
                match follower {
                    Ok(follower) => {
                        let inserted = follows::Follow::insert(
                            conn,
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

fn fetch_and_cache_user(user: &Arc<User>, conn: &DbConn) {
    if user.refetch(conn).is_err() {
        error!("Couldn't update user info: {:?}", user);
    }
}
