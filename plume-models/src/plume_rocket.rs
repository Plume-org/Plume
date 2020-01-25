pub use self::module::PlumeRocket;

#[cfg(not(test))]
mod module {
    use crate::{db_conn::DbConn, search, users};
    use rocket::{
        request::{self, FlashMessage, FromRequest, FromRequestAsync, Request},
        Outcome, State,
    };
    use scheduled_thread_pool::ScheduledThreadPool;
    use std::sync::Arc;

    /// Common context needed by most routes and operations on models
    pub struct PlumeRocket {
        pub conn: DbConn,
        pub intl: rocket_i18n::I18n,
        pub user: Option<users::User>,
        pub searcher: Arc<search::Searcher>,
        pub worker: Arc<ScheduledThreadPool>,
        pub flash_msg: Option<(String, String)>,
    }

    impl<'a, 'r> FromRequestAsync<'a, 'r> for PlumeRocket {
        type Error = ();

        fn from_request(request: &'a Request<'r>) -> request::FromRequestFuture<'a, Self, Self::Error> {
            Box::pin(async move {
                let conn = try_outcome!(DbConn::from_request(request).await);
                let intl = try_outcome!(rocket_i18n::I18n::from_request(request).await);
                let user = try_outcome!(users::User::from_request(request).await);
                let worker = request.guard::<'_, State<'_, Arc<ScheduledThreadPool>>>()?;
                let searcher = request.guard::<'_, State<'_, Arc<search::Searcher>>>()?;
                let flash_msg = request.guard::<FlashMessage<'_, '_>>().succeeded();
                Outcome::Success(PlumeRocket {
                    conn,
                    intl,
                    user,
                    flash_msg: flash_msg.map(|f| (f.name().into(), f.msg().into())),
                    worker: worker.clone(),
                    searcher: searcher.clone(),
                })
            })
        }
    }
}

#[cfg(test)]
mod module {
    use crate::{db_conn::DbConn, search, users};
    use rocket::{
        request::{self, FromRequest, Request},
        Outcome, State,
    };
    use scheduled_thread_pool::ScheduledThreadPool;
    use std::sync::Arc;

    /// Common context needed by most routes and operations on models
    pub struct PlumeRocket {
        pub conn: DbConn,
        pub user: Option<users::User>,
        pub searcher: Arc<search::Searcher>,
        pub worker: Arc<ScheduledThreadPool>,
    }

    impl<'a, 'r> FromRequest<'a, 'r> for PlumeRocket {
        type Error = ();

        fn from_request(request: &'a Request<'r>) -> request::Outcome<PlumeRocket, ()> {
            let conn = DbConn::from_request(request).await;
            let user = request.guard::<users::User>().succeeded();
            let worker = request.guard::<'_, State<'_, Arc<ScheduledThreadPool>>>()?;
            let searcher = request.guard::<'_, State<'_, Arc<search::Searcher>>>()?;
            Outcome::Success(PlumeRocket {
                conn,
                user,
                worker: worker.clone(),
                searcher: searcher.clone(),
            })
        }
    }
}
