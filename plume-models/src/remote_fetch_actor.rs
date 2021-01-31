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
    for create_act in user
        .fetch_outbox::<Create>()
        .expect("Remote user: outbox couldn't be fetched")
    {
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

fn fetch_and_cache_followers(user: &Arc<User>, conn: &DbConn) {
    for user_id in user
        .fetch_followers_ids()
        .expect("Remote user: fetching followers error")
    {
        let follower = User::from_id(conn, &user_id, None, CONFIG.proxy())
            .expect("user::details: Couldn't fetch follower");
        follows::Follow::insert(
            conn,
            follows::NewFollow {
                follower_id: follower.id,
                following_id: user.id,
                ap_url: String::new(),
            },
        )
        .expect("Couldn't save follower for remote user");
    }
}

fn fetch_and_cache_user(user: &Arc<User>, conn: &DbConn) {
    user.refetch(conn).expect("Couldn't update user info");
}
