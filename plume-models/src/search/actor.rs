use super::Searcher;
use crate::{db_conn::DbPool, posts::PostEvent, ACTOR_SYS, POST_CHAN};
use riker::actors::{Actor, ActorFactoryArgs, ActorRefFactory, Context, Sender, Subscribe, Tell};
use std::sync::Arc;
use tracing::error;

pub struct SearchActor {
    searcher: Arc<Searcher>,
    conn: DbPool,
}

impl SearchActor {
    pub fn init(searcher: Arc<Searcher>, conn: DbPool) {
        ACTOR_SYS
            .actor_of_args::<SearchActor, _>("search", (searcher, conn))
            .expect("Failed to initialize searcher actor");
    }
}

impl Actor for SearchActor {
    type Msg = PostEvent;

    fn pre_start(&mut self, ctx: &Context<Self::Msg>) {
        POST_CHAN.tell(
            Subscribe {
                actor: Box::new(ctx.myself()),
                topic: "*".into(),
            },
            None,
        )
    }

    fn recv(&mut self, _ctx: &Context<Self::Msg>, msg: Self::Msg, _sender: Sender) {
        use PostEvent::*;

        match msg {
            PostPublished(post) => {
                let conn = self.conn.get();
                match conn {
                    Ok(_) => {
                        self.searcher
                            .add_document(&conn.unwrap(), &post)
                            .unwrap_or_else(|e| error!("{:?}", e));
                    }
                    _ => {
                        error!("Failed to get database connection");
                    }
                }
            }
            PostUpdated(post) => {
                let conn = self.conn.get();
                match conn {
                    Ok(_) => {
                        self.searcher
                            .update_document(&conn.unwrap(), &post)
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
